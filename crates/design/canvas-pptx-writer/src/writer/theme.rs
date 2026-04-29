pub fn build() -> Vec<u8> {
    include_str!("boilerplate/theme1.xml").as_bytes().to_vec()
}

#[cfg(test)]
mod tests {
    #[test]
    fn theme_starts_with_xml_decl() {
        let s = String::from_utf8(super::build()).unwrap();
        assert!(s.starts_with(r#"<?xml"#));
        assert!(s.contains("<a:theme"));
    }
}
