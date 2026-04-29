pub fn build() -> Vec<u8> {
    include_str!("boilerplate/slide_master1.xml").as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    #[test]
    fn master_is_valid_xml_declaration() {
        let s = String::from_utf8(super::build()).unwrap();
        assert!(s.contains("<p:sldMaster"));
    }
}
