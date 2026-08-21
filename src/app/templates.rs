use crate::app::assets;

pub fn init() -> minijinja::Environment<'static> {
    let mut templates = minijinja::Environment::new();
    templates.set_loader(minijinja::path_loader("templates"));
    templates.set_auto_escape_callback(|name| {
        if name.ends_with(".html") {
            minijinja::AutoEscape::Html
        } else {
            minijinja::AutoEscape::None
        }
    });
    templates.add_function("asset_url", |file: String| {
        Ok::<minijinja::Value, minijinja::Error>(minijinja::Value::from_safe_string(
            assets::asset_url(&file),
        ))
    });
    templates
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The live HTTP test only renders `home.html`, so the non-`.html`
    /// else-branch of the auto-escape callback needs its own coverage.
    /// There is no public accessor for the callback, so assert the rendered
    /// behavior instead: `.html` names escape, other names do not.
    #[test]
    fn html_names_are_escaped_and_others_are_not() {
        let mut env = init();
        env.add_template("note.html", "{{ value }}").unwrap();
        env.add_template("note.txt", "{{ value }}").unwrap();

        let html = env
            .get_template("note.html")
            .unwrap()
            .render(minijinja::context! { value => "<b>" })
            .unwrap();
        assert_eq!(html, "&lt;b&gt;");

        let txt = env
            .get_template("note.txt")
            .unwrap()
            .render(minijinja::context! { value => "<b>" })
            .unwrap();
        assert_eq!(txt, "<b>");
    }

    #[test]
    fn asset_url_function_resolves_in_templates() {
        let mut env = init();
        env.add_template("page.html", r"{{ asset_url('site.css') }}")
            .unwrap();
        let out = env
            .get_template("page.html")
            .unwrap()
            .render(minijinja::context! {})
            .unwrap();
        assert!(out.starts_with("/static/site.css?v="));
    }
}
