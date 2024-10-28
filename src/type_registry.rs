use std::fs;

pub struct UserTypeRegistry;

impl UserTypeRegistry {

    pub fn from_ron_file(file_path: &str) -> Vec<String> {

        let input_contents = fs::read_to_string(file_path);

        let binding = input_contents.expect("REASON");
        let sections: Vec<&str> = binding.split("ty: DataTable").collect();

        sections
        .into_iter()
        .enumerate()
        .filter_map(|(i, section)| {
            if i == 0 && section.trim().is_empty() {
                None
            } else {
                Some(format!("ty: DataTable{}", section))
            }
        })
        .collect()
    }
}
