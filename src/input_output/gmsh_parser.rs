use std::fs::File;
use std::io::{self, BufRead, BufReader};
use mesh_sieve::InMemorySieve;

pub struct GmshParser;

impl GmshParser {
    /// Load the mesh from a Gmsh file and build topology using mesh-sieve
    pub fn from_gmsh_file(file_path: &str) -> Result<Mesh, io::Error> {
        let file = File::open(file_path)?;
        let reader = BufReader::new(file);

        let mut sieve = InMemorySieve::<usize, ()>::new();
        let mut in_nodes_section = false;
        let mut in_elements_section = false;

        let mut node_count = 0;
        let mut element_count = 0;
        let mut current_node_line = 0;
        let mut current_element_line = 0;

        for line in reader.lines() {
            let line = line?;

            if line.starts_with("$Nodes") {
                in_nodes_section = true;
                in_elements_section = false;
                current_node_line = 0;
                continue;
            } else if line.starts_with("$Elements") {
                in_nodes_section = false;
                in_elements_section = true;
                current_element_line = 0;
                continue;
            } else if line.starts_with("$EndNodes") || line.starts_with("$EndElements") {
                in_nodes_section = false;
                in_elements_section = false;
                continue;
            }

            // Parse node and element counts
            if in_nodes_section && current_node_line == 0 {
                node_count = line.parse::<usize>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid node count"))?;
                current_node_line += 1;
                continue;
            } else if in_elements_section && current_element_line == 0 {
                element_count = line.parse::<usize>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid element count"))?;
                current_element_line += 1;
                continue;
            }

            // Parse individual nodes
            if in_nodes_section && current_node_line <= node_count {
                let (vertex_id, coords) = Self::parse_node(&line)?;
                mesh.set_vertex_coordinates(vertex_id, coords).unwrap();
                sieve.add_cap_point(vertex_id); // Register as a cap point (vertex)
                current_node_line += 1;
            }

            // Parse individual elements
            if in_elements_section && current_element_line <= element_count {
                if let Some((element_id, node_ids)) = Self::parse_element(&line)? {
                    mesh.add_entity(MeshEntity::Cell(element_id)).unwrap();
                    sieve.add_base_point(element_id); // Register as a base point (cell)
                    for &node_id in &node_ids {
                        sieve.add_arrow(element_id, node_id, ()); // Cell → Vertex incidence
                    }
                }
                current_element_line += 1;
            }
        }

        // Optionally, attach the sieve to the mesh or return both
        // mesh.set_topology(sieve); // If mesh supports this
        Ok(mesh)
    }

    /// Parse a single node from a line in the Gmsh file
    fn parse_node(line: &str) -> Result<(usize, [f64; 3]), io::Error> {
        let mut split = line.split_whitespace();
        let id: usize = Self::parse_next(&mut split, "Missing node ID")?;
        let x: f64 = Self::parse_next(&mut split, "Missing x coordinate")?;
        let y: f64 = Self::parse_next(&mut split, "Missing y coordinate")?;
        let z: f64 = Self::parse_next(&mut split, "Missing z coordinate")?;

        Ok((id, [x, y, z]))
    }

    /// Parse an element from a line in the Gmsh file
    fn parse_element(line: &str) -> Result<Option<(usize, Vec<usize>)>, io::Error> {
        let mut split = line.split_whitespace();

        let id: usize = Self::parse_next(&mut split, "Missing element ID")?;
        let element_type: u32 = Self::parse_next(&mut split, "Missing element type")?;

        // Only process triangular elements (type `2`) and quadrilateral elements (type `3`)
        let expected_nodes = match element_type {
            2 => 3, // Triangle has 3 nodes
            3 => 4, // Quadrilateral has 4 nodes
            _ => return Ok(None), // Skip unsupported element types
        };

        // Skip the number of tags and all tag data
        let num_tags: u32 = Self::parse_next(&mut split, "Missing number of tags")?;
        for _ in 0..num_tags {
            let _ = Self::parse_next::<u32, _>(&mut split, "Missing tag data")?;
        }

        // Parse exactly `expected_nodes` node IDs
        let node_ids: Vec<usize> = split
            .take(expected_nodes)
            .map(|s| s.parse::<usize>())
            .collect::<Result<Vec<_>, _>>()
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err))?;

        // Check if the correct number of nodes were parsed
        if node_ids.len() != expected_nodes {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Incorrect node count for element"));
        }

        Ok(Some((id, node_ids)))
    }

    /// Utility function to parse the next value from an iterator
    fn parse_next<'a, T: std::str::FromStr, I: Iterator<Item = &'a str>>(
        iter: &mut I,
        err_msg: &str,
    ) -> Result<T, io::Error>
    where
        T::Err: std::fmt::Debug,
    {
        iter.next()
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, err_msg))?
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, err_msg))
    }
}


#[cfg(test)]
mod tests {

    use crate::input_output::gmsh_parser::GmshParser;

    #[test]
    fn test_parse_node() {
        let line = "3 5 8.66 0";
        let (id, coords) = GmshParser::parse_node(line).unwrap();
        assert_eq!(id, 3);
        assert_eq!(coords, [5.0, 8.66, 0.0]);
    }

    #[test]
    fn test_parse_element_triangle() {
        let line = "58 2 2 0 1 22 23 40";
        let result = GmshParser::parse_element(line).unwrap();

        assert!(result.is_some(), "Expected a valid element, but got None");
        let (id, nodes) = result.unwrap();
        assert_eq!(id, 58);
        assert_eq!(nodes, vec![22, 23, 40]); // Should be exactly 3 nodes for a triangle
    }

    #[test]
    fn test_parse_element_quadrilateral() {
        let line = "60 3 2 0 1 11 12 65 35";
        let result = GmshParser::parse_element(line).unwrap();

        assert!(result.is_some(), "Expected a valid element, but got None");
        let (id, nodes) = result.unwrap();
        assert_eq!(id, 60);
        assert_eq!(nodes, vec![11, 12, 65, 35]); // Should be exactly 4 nodes for a quadrilateral
    }

    #[test]
    fn test_circle_mesh_import() {
        let temp_file_path = "inputs/circular_lake.msh2";
        let result = GmshParser::from_gmsh_file(temp_file_path);
        assert!(result.is_ok());

        let mesh = result.unwrap();
        assert_eq!(mesh.get_cells().len(), 780);
    }

    // Similar tests for other meshes, e.g., rectangular_channel.msh2, etc.
}
