use super::producer::Producer;

impl Producer {
    pub fn writeln_json<T: serde::Serialize>(&mut self, data: &T) -> std::io::Result<usize> {
        let json_data = serde_json::to_string(data)?;
        self.write(json_data.as_bytes())
    }
}
