pub fn build() -> Vec<u8> {
    include_str!("boilerplate/slide_layout1.xml")
        .as_bytes()
        .to_vec()
}

#[cfg(test)]
mod tests {
    #[test]
    fn layout_is_blank_type() {
        let s = String::from_utf8(super::build()).unwrap();
        assert!(s.contains(r#"type="blank""#));
    }
}
