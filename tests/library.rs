//! The crate used the way another Rust CLI or app would use it: through its
//! public API only, with no command line and no files on disk.

use mhtml::{CssSource, Options, Page, Theme, render, to_html};

#[test]
fn renders_a_complete_self_contained_page() {
    let doc = to_html("# Hello\n\nworld", &Options::default()).unwrap();

    assert!(doc.html.starts_with("<!doctype html>"), "{}", doc.html);
    assert!(doc.html.trim_end().ends_with("</html>"), "{}", doc.html);
    assert!(doc.html.contains("<title>Hello</title>"));
    assert!(doc.html.contains("<main class=\"markdown-body\">"));
    assert_eq!(doc.title, "Hello");
    assert_eq!(doc.lang, "en");
    assert!(!doc.has_warnings());
    // Self-contained: nothing is fetched from the network.
    assert!(!doc.html.contains("src=\"http"));
    assert!(!doc.html.contains("<link rel=\"stylesheet\" href=\"http"));
}

#[test]
fn options_steer_the_page() {
    let options = Options::default()
        .with_title("Notes")
        .with_lang("zh-TW")
        .with_theme(Theme::Dark)
        .with_toc("目錄");

    let doc = to_html("# 標題\n\n## 第一節\n\ntext", &options).unwrap();
    assert!(
        doc.html
            .contains(r#"<html lang="zh-TW" data-theme="dark">"#),
        "{}",
        doc.html
    );
    assert!(doc.html.contains("<title>Notes</title>"));
    assert!(doc.html.contains("目錄"));
}

#[test]
fn caller_css_is_added_last() {
    let options = Options::default().with_css(CssSource::Inline {
        name: "app.css".into(),
        css: ".brand { color: rebeccapurple }".into(),
    });

    let doc = to_html("# Title", &options).unwrap();
    let built_in = doc.html.find("id=\"mhtml-base\"").expect("built-in CSS");
    let caller = doc.html.find("rebeccapurple").expect("caller CSS");
    assert!(built_in < caller, "caller CSS must win the cascade");

    let linked = Options::default().with_css(CssSource::Link("https://example.test/a.css".into()));
    assert!(
        to_html("x", &linked)
            .unwrap()
            .html
            .contains("<link rel=\"stylesheet\" href=\"https://example.test/a.css\">")
    );
}

#[test]
fn untrusted_markdown_is_sanitized_by_default() {
    let md = "<script>alert(1)</script>\n\n<img src=x onerror=\"alert(2)\">";
    let safe = to_html(md, &Options::default()).unwrap();
    assert!(!safe.html.contains("<script>alert(1)"), "{}", safe.html);
    assert!(!safe.html.contains("onerror"), "{}", safe.html);
}

#[test]
fn a_bad_styling_shortcut_is_an_error_not_a_broken_page() {
    let options = Options {
        accent: Some("red; } body { display: none".into()),
        ..Options::default()
    };
    let err = to_html("# x", &options).unwrap_err();
    assert_eq!(err.name, "accent");
    assert!(err.to_string().contains("expected a plain CSS value"));
}

#[test]
fn the_layers_can_be_used_separately() {
    // Body only, for a page of the caller's own.
    let rendered = render::render("# Hi", &render::RenderOptions::default());
    assert!(rendered.body.contains("<h1"), "{}", rendered.body);

    // Or a completely different body through the page builder.
    let html = Page {
        title: "Custom",
        lang: "en",
        body: "<p>hand-made</p>",
        theme: Theme::Light,
        base: None,
        default_style: false,
        enhance: false,
        main_class: "",
        outline: None,
        interactive: false,
        title_banner: false,
        highlight: false,
        mermaid: false,
        overrides: None,
        extra_css: &[],
    }
    .to_html();
    assert!(html.contains("<p>hand-made</p>"), "{html}");
    assert!(html.contains("data-theme=\"light\""));
}

#[test]
fn the_document_renders_as_html_through_display() {
    let doc = to_html("# A", &Options::default()).unwrap();
    assert_eq!(doc.to_string(), doc.html);
}
