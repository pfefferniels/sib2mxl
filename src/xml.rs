//! A small XML writer whose elements close themselves.

pub struct Xml {
    out: String,
    depth: usize,
}

impl Xml {
    /// A document whose root element is described by `doctype`.
    pub fn new(doctype: &str) -> Self {
        let out = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{doctype}\n");
        Self { out, depth: 0 }
    }

    pub fn element(&mut self, name: &str, attributes: &[(&str, &str)], body: impl FnOnce(&mut Self)) {
        self.open_tag(name, attributes);
        self.out.push_str(">\n");
        self.depth += 1;
        body(self);
        self.depth -= 1;
        self.indent();
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    pub fn text(&mut self, name: &str, attributes: &[(&str, &str)], text: &str) {
        self.open_tag(name, attributes);
        self.out.push('>');
        self.out.push_str(&escape(text));
        self.out.push_str("</");
        self.out.push_str(name);
        self.out.push_str(">\n");
    }

    pub fn empty(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.open_tag(name, attributes);
        self.out.push_str("/>\n");
    }

    pub fn finish(self) -> String {
        self.out
    }

    fn open_tag(&mut self, name: &str, attributes: &[(&str, &str)]) {
        self.indent();
        self.out.push('<');
        self.out.push_str(name);
        attributes.iter().for_each(|(key, value)| {
            self.out.push(' ');
            self.out.push_str(key);
            self.out.push_str("=\"");
            self.out.push_str(&escape(value));
            self.out.push('"');
        });
    }

    fn indent(&mut self) {
        (0..self.depth).for_each(|_| self.out.push_str("  "));
    }
}

fn escape(text: &str) -> String {
    text.chars().fold(String::with_capacity(text.len()), |mut out, c| {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c if c.is_control() && c != '\n' && c != '\t' => {}
            c => out.push(c),
        }
        out
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nests_and_escapes() {
        let mut xml = Xml::new("<!DOCTYPE a>");
        xml.element("a", &[("b", "x\"y")], |xml| xml.text("c", &[], "1 < 2 & 3"));
        assert_eq!(
            xml.finish(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE a>\n<a b=\"x&quot;y\">\n  <c>1 &lt; 2 &amp; 3</c>\n</a>\n"
        );
    }
}
