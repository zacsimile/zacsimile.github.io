use std::collections::BTreeMap;
use std::env;
use std::fmt::Write as _;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const SITE_URL: &str = "https://zacsimile.github.io";

const NAV_ITEMS: [(&str, &str, &str); 3] = [
    ("about", "index.html", "About me"),
    ("research", "research.html", "Research"),
    ("blog", "blog.html", "Blog"),
];

const SOCIAL_LINKS: [(&str, &str); 5] = [
    ("https://orcid.org/0000-0001-5341-9911", "ORCID"),
    (
        "https://scholar.google.com/citations?user=pSS31d8AAAAJ&hl=en",
        "Google Scholar",
    ),
    ("https://bsky.app/profile/zacsimile.bsky.social", "Bluesky"),
    ("https://github.com/zacsimile", "GitHub"),
    ("https://www.linkedin.com/in/zach-marin/", "LinkedIn"),
];

const PAGE_TEMPLATE: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{title}}</title>
    <link rel="preload" as="image" href="images/4pi_illumination_compressed.jpg">
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Open+Sans:ital,wght@0,400;0,600;0,700;1,400;1,700&amp;display=swap">
    <link rel="stylesheet" href="css/main.css">
    <link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/jpswalsh/academicons@1/css/academicons.min.css">
    <script src="https://kit.fontawesome.com/0ffd6a402a.js" crossorigin="anonymous"></script>
    <link rel="alternate" type="application/rss+xml" title="Zach Marin's blog" href="rss.xml">
{{extra_head}}</head>
<body>
    <a class="skip-link" href="#main-content">Skip to content</a>
{{header}}    <main id="main-content" tabindex="-1">
{{body}}
    </main>
    <footer>{{footer}}</footer>
</body>
</html>
"##;

const BARE_TEMPLATE: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{{title}}</title>
    <link rel="preload" as="image" href="images/4pi_illumination_compressed.jpg">
    <link rel="preconnect" href="https://fonts.googleapis.com">
    <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin>
    <link rel="stylesheet" href="https://fonts.googleapis.com/css2?family=Open+Sans:ital,wght@0,400;0,600;0,700;1,400;1,700&amp;display=swap">
    <link rel="stylesheet" href="css/main.css">
</head>
<body>
    <main>
{{body}}
    </main>
</body>
</html>
"#;

const MATHJAX_HEAD: &str = r#"    <script>
        window.MathJax = {
            tex: {
                inlineMath: [['\\(', '\\)']],
                displayMath: [['$$', '$$'], ['\\[', '\\]']]
            }
        };
    </script>
    <script async src="https://cdn.jsdelivr.net/npm/mathjax@3/es5/tex-chtml.js"></script>
"#;

#[derive(Debug)]
struct Page {
    output: String,
    title: String,
    nav: Option<String>,
    category: Option<String>,
    date: Option<String>,
    date_modified: Option<String>,
    summary: Option<String>,
    mathjax: bool,
    bare: bool,
    footer: String,
    body: String,
}

/// Runs the generator and reports failures to standard error.
fn main() {
    if let Err(error) = run() {
        eprintln!("guccify: {error}");
        std::process::exit(1);
    }
}

/// Parses command-line options and generates or checks every output file.
fn run() -> Result<(), String> {
    let project = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut content = project.join("content");
    let mut output = project.parent().unwrap_or(&project).to_path_buf();
    let mut check = false;
    let mut args = env::args().skip(1);

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--content" => content = required_path(&mut args, "--content")?,
            "--output" => output = required_path(&mut args, "--output")?,
            "--check" => check = true,
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            _ => return Err(format!("unknown argument {arg:?}; try --help")),
        }
    }

    let pages = read_pages(&content)?;
    let blog_posts = blog_posts(&pages)?;
    let mut files = pages
        .iter()
        .map(|page| (page.output.clone(), render(page)))
        .collect::<Vec<_>>();
    files.push(("blog.html".into(), render_blog_index(&blog_posts)));
    files.push(("rss.xml".into(), render_rss(&blog_posts)));

    let mut changed = 0;
    for (relative_path, rendered) in &files {
        let destination = output.join(relative_path);
        if fs::read_to_string(&destination).ok().as_deref() == Some(rendered.as_str()) {
            println!("unchanged {}", destination.display());
            continue;
        }
        changed += 1;
        if check {
            println!("would update {}", destination.display());
        } else {
            write_atomic(&destination, rendered.as_bytes())
                .map_err(|e| format!("cannot write {}: {e}", destination.display()))?;
            println!("generated {}", destination.display());
        }
    }

    if check && changed > 0 {
        return Err(format!("{changed} generated file(s) are out of date"));
    }
    println!("done: {} file(s), {changed} changed", files.len());
    Ok(())
}

/// Reads the path argument that must follow a command-line option.
fn required_path(args: &mut impl Iterator<Item = String>, option: &str) -> Result<PathBuf, String> {
    args.next()
        .map(PathBuf::from)
        .ok_or_else(|| format!("{option} needs a path"))
}

/// Prints the command-line usage summary.
fn print_help() {
    println!("guccify - the static site generator for zacsimile.github.io\n");
    println!("Usage: cargo run -- [--content DIR] [--output DIR] [--check]");
    println!("Defaults: content/ beside Cargo.toml; output to the repository root.");
}

/// Loads and parses every HTML content file in the content directory.
fn read_pages(content: &Path) -> Result<Vec<Page>, String> {
    let entries =
        fs::read_dir(content).map_err(|e| format!("cannot read {}: {e}", content.display()))?;
    let mut paths = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|s| s.to_str()) == Some("html"))
        .collect::<Vec<_>>();
    paths.sort();
    paths.into_iter().map(|path| parse_page(&path)).collect()
}

/// Parses one metadata header and its following HTML body into a page.
fn parse_page(path: &Path) -> Result<Page, String> {
    let text =
        fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let (metadata, body) = text
        .split_once("\n---\n")
        .ok_or_else(|| format!("{} has no metadata delimiter (---)", path.display()))?;
    let mut fields = BTreeMap::new();
    for (line_number, line) in metadata.lines().enumerate() {
        let (key, value) = line.split_once(':').ok_or_else(|| {
            format!(
                "{}:{}: expected key: value",
                path.display(),
                line_number + 1
            )
        })?;
        fields.insert(key.trim().to_owned(), value.trim().to_owned());
    }
    let take = |key: &str| fields.get(key).cloned();
    let required =
        |key: &str| take(key).ok_or_else(|| format!("{}: missing {key}", path.display()));
    Ok(Page {
        output: required("output")?,
        title: required("title")?,
        nav: take("nav").filter(|value| !value.is_empty()),
        category: take("category").filter(|value| !value.is_empty()),
        date: take("date").filter(|value| !value.is_empty()),
        date_modified: take("date_modified").filter(|value| !value.is_empty()),
        summary: take("summary").filter(|value| !value.is_empty()),
        mathjax: take("mathjax").as_deref() == Some("true"),
        bare: take("layout").as_deref() == Some("bare"),
        footer: take("footer")
            .unwrap_or_else(|| "Copyright &copy; 2026 Zachary Ryan Connerty-Marin.".into()),
        body: body.trim().to_owned(),
    })
}

/// Selects, validates, and newest-first sorts pages categorized as blog posts.
fn blog_posts(pages: &[Page]) -> Result<Vec<&Page>, String> {
    let mut posts = pages
        .iter()
        .filter(|page| page.category.as_deref() == Some("blog"))
        .collect::<Vec<_>>();
    for post in &posts {
        if post.date.as_deref().map(valid_iso_date) != Some(true) {
            return Err(format!(
                "blog post {} needs a date in YYYY-MM-DD form",
                post.output
            ));
        }
        if let Some(modified) = post.date_modified.as_deref() {
            if !valid_iso_date(modified) {
                return Err(format!(
                    "blog post {} needs date_modified to be a valid date in YYYY-MM-DD form",
                    post.output
                ));
            }
            if modified < post.date.as_deref().unwrap() {
                return Err(format!(
                    "blog post {} has date_modified before its creation date",
                    post.output
                ));
            }
        }
        if post.summary.is_none() {
            return Err(format!("blog post {} needs a summary", post.output));
        }
    }
    posts.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| a.title.cmp(&b.title)));
    Ok(posts)
}

/// Checks the ISO shape and calendar validity before formatting dates.
fn valid_iso_date(date: &str) -> bool {
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || !bytes
            .iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }
    let year = date[..4].parse::<u32>().unwrap();
    let month = date[5..7].parse::<usize>().unwrap();
    let day = date[8..10].parse::<u32>().unwrap();
    if year == 0 || !(1..=12).contains(&month) {
        return false;
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days = [
        31,
        if leap { 29 } else { 28 },
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    (1..=days[month - 1]).contains(&day)
}

/// Renders a page with either the standard or bare HTML template.
fn render(page: &Page) -> String {
    let page_body = render_page_body(page);
    if page.bare {
        return fill_template(
            BARE_TEMPLATE,
            &[("title", &page.title), ("body", &indent(&page_body, 8))],
        );
    }
    fill_template(
        PAGE_TEMPLATE,
        &[
            ("title", &page.title),
            ("extra_head", if page.mathjax { MATHJAX_HEAD } else { "" }),
            (
                "header",
                &header(if page.category.as_deref() == Some("blog") {
                    Some("blog")
                } else {
                    page.nav.as_deref()
                }),
            ),
            ("body", &indent(&page_body, 8)),
            ("footer", &page.footer),
        ],
    )
}

/// Adds creation and optional modification dates to a blog post.
fn render_page_body(page: &Page) -> String {
    let Some(date) = page.date.as_deref() else {
        return page.body.clone();
    };
    if page.category.as_deref() != Some("blog") {
        return page.body.clone();
    }

    let mut date_line = format!(
        "<p class=\"post-date\">Created <time datetime=\"{date}\">{}</time>",
        human_date(date)
    );
    if let Some(modified) = page.date_modified.as_deref() {
        write!(
            date_line,
            "<br>Last modified <time datetime=\"{modified}\">{}</time>",
            human_date(modified)
        )
        .unwrap();
    }
    date_line.push_str("</p>");
    format!("{}\n{date_line}", page.body)
}

/// Builds the blog listing page from the dated blog posts.
fn render_blog_index(posts: &[&Page]) -> String {
    let mut body = String::from("<h1>Blog</h1>\n<p class=\"lede\">Writing about imaging, mathematics, and other things that catch my interest.</p>\n<ul class=\"blog-posts\">\n");
    for post in posts {
        let date = post.date.as_deref().unwrap();
        let summary: &str = post.summary.as_deref().unwrap();
        writeln!(
            body,
            "    <li><time datetime=\"{date}\">{}</time><h2><a href=\"{}\">{}</a></h2><p>{}</p></li>",
            human_date(date),
            post.output,
            post.title_without_site(),
            summary
        )
        .unwrap();
    }
    body.push_str("</ul>\n<p><a href=\"rss.xml\" class=\"publication\">Subscribe via RSS</a></p>");
    let page = Page {
        output: "blog.html".into(),
        title: "Zach Marin - Blog".into(),
        nav: Some("blog".into()),
        category: None,
        date: None,
        date_modified: None,
        summary: None,
        mathjax: false,
        bare: false,
        footer: "Copyright &copy; 2026 Zachary Ryan Connerty-Marin.".into(),
        body,
    };
    render(&page)
}

/// Builds an RSS 2.0 document containing all blog posts.
fn render_rss(posts: &[&Page]) -> String {
    let mut rss = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<rss version="2.0">
  <channel>
    <title>Zach Marin's blog</title>
    <link>{SITE_URL}/blog.html</link>
    <description>Notes, experiments, and miscellaneous investigations by Zach Marin.</description>
    <language>en</language>
"#
    );
    for post in posts {
        let date = post.date.as_deref().unwrap();
        let url = format!("{SITE_URL}/{}", post.output);
        writeln!(rss, "    <item>\n      <title>{}</title>\n      <link>{url}</link>\n      <guid>{url}</guid>\n      <pubDate>{}</pubDate>\n      <description>{}</description>\n    </item>", xml_escape(post.title_without_site()), rss_date(date), xml_escape(post.summary.as_deref().unwrap())).unwrap();
    }
    rss.push_str("  </channel>\n</rss>\n");
    rss
}

impl Page {
    /// Returns the page title without the site's shared title prefix.
    fn title_without_site(&self) -> &str {
        self.title
            .strip_prefix("Zach Marin - ")
            .unwrap_or(&self.title)
    }
}

/// Builds the shared header and marks the active navigation item.
fn header(active: Option<&str>) -> String {
    let mut html = String::from(
        r#"    <header>
        <a class="site-name" href="index.html">Zach Marin</a>
        <nav aria-label="Primary navigation">
            <ul>
"#,
    );
    for (key, href, label) in NAV_ITEMS {
        if active == Some(key) {
            writeln!(html, "                <li><strong><a href=\"{href}\" aria-current=\"page\">{label}</a></strong></li>").unwrap();
        } else {
            writeln!(
                html,
                "                <li><a href=\"{href}\">{label}</a></li>"
            )
            .unwrap();
        }
    }
    html.push_str("            </ul>\n        </nav>\n");
    html.push_str(&social_links());
    html.push_str("    </header>\n");
    html
}

/// Builds the original header profile icons with accessible link names.
fn social_links() -> String {
    let mut html =
        String::from("<nav aria-label=\"Elsewhere\" class=\"social-links\"><ul class=\"icons\">");
    for (href, label) in SOCIAL_LINKS {
        let class = match label {
            "ORCID" => "ai ai-orcid",
            "Google Scholar" => "ai ai-google-scholar",
            "Bluesky" => "fa fa-brands fa-bluesky",
            "GitHub" => "fa fa-github",
            "LinkedIn" => "fa fa-linkedin",
            _ => "",
        };
        write!(html, "<li><a href=\"{href}\" aria-label=\"{label}\" title=\"{label}\"><i class=\"{class}\" aria-hidden=\"true\"></i></a></li>").unwrap();
    }
    html.push_str("</ul></nav>\n");
    html
}

/// Replaces named placeholders in a small, human-readable HTML template.
fn fill_template(template: &str, values: &[(&str, &str)]) -> String {
    values
        .iter()
        .fold(template.to_owned(), |result, (key, value)| {
            result.replace(&format!("{{{{{key}}}}}"), value)
        })
}

/// Converts an ISO date into a date suitable for display on the website.
fn human_date(date: &str) -> String {
    const MONTHS: [&str; 12] = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month = date[5..7].parse::<usize>().unwrap();
    let day = date[8..10].parse::<u8>().unwrap();
    format!("{} {day}, {}", MONTHS[month - 1], &date[..4])
}

/// Converts an ISO date into the RFC 822-style form required by RSS.
fn rss_date(date: &str) -> String {
    // Zeller's congruence numbers weekdays from Saturday through Friday.
    let days = ["Sat", "Sun", "Mon", "Tue", "Wed", "Thu", "Fri"];
    let year = date[..4].parse::<i32>().unwrap();
    let month = date[5..7].parse::<i32>().unwrap();
    let day = date[8..10].parse::<i32>().unwrap();
    let (y, m) = if month < 3 {
        (year - 1, month + 12)
    } else {
        (year, month)
    };
    let weekday = (day + (13 * (m + 1)) / 5 + y + y / 4 - y / 100 + y / 400).rem_euclid(7) as usize;
    let months = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    format!(
        "{}, {:02} {} {} 00:00:00 GMT",
        days[weekday],
        day,
        months[month as usize - 1],
        year
    )
}

/// Escapes characters that have special meaning in XML text.
fn xml_escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Indents HTML without altering whitespace inside preformatted code.
fn indent(text: &str, spaces: usize) -> String {
    let prefix = " ".repeat(spaces);
    let mut in_pre = false;
    text.lines()
        .map(|line| {
            let preserve = in_pre;
            if line.contains("<pre>") || line.contains("<pre ") {
                in_pre = true;
            }
            if line.contains("</pre>") {
                in_pre = false;
            }
            if preserve {
                return line.to_owned();
            }
            let line = line.trim_start();
            if line.is_empty() {
                String::new()
            } else {
                format!("{prefix}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Writes through a temporary file to avoid leaving partial output behind.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_preformatted_code_indentation() {
        let body = "<pre tabindex=\"0\"><code>def example():\n    return 1\n\nprint(example())\n</code></pre>\n<p>After.</p>";
        let rendered = indent(body, 8);
        assert!(rendered
            .contains("<code>def example():\n    return 1\n\nprint(example())\n</code></pre>"));
        assert!(rendered.ends_with("\n        <p>After.</p>"));
    }

    #[test]
    /// Verifies that exactly one navigation link is marked as active.
    fn active_navigation_is_marked() {
        let html = header(Some("blog"));
        assert!(html.contains("href=\"blog.html\" aria-current=\"page\""));
        assert_eq!(html.matches("aria-current").count(), 1);
    }

    #[test]
    /// Verifies both human-readable and RSS date formatting.
    fn formats_dates() {
        assert_eq!(human_date("2026-01-18"), "January 18, 2026");
        assert_eq!(rss_date("2026-01-18"), "Sun, 18 Jan 2026 00:00:00 GMT");
    }

    #[test]
    fn validates_calendar_dates() {
        for date in ["2024-02-29", "2026-09-29"] {
            assert!(valid_iso_date(date));
        }
        for date in [
            "2026-02-29",
            "2026-13-01",
            "2026-00-01",
            "2026-01-00",
            "2026-04-31",
            "September 29, 2026",
        ] {
            assert!(!valid_iso_date(date));
        }
    }

    #[test]
    /// Verifies that reserved XML characters are escaped.
    fn escapes_xml() {
        assert_eq!(xml_escape("cats & <math>"), "cats &amp; &lt;math&gt;");
    }

    #[test]
    /// Verifies that blog dates follow the complete post body.
    fn blog_dates_are_shown_after_body() {
        let mut page = Page {
            output: "post.html".into(),
            title: "Post".into(),
            nav: None,
            category: Some("blog".into()),
            date: Some("2026-01-18".into()),
            date_modified: None,
            summary: Some("Summary".into()),
            mathjax: false,
            bare: false,
            footer: String::new(),
            body: "<h2>Post</h2>\n<p>Words.</p>".into(),
        };
        assert!(render_page_body(&page).starts_with(
            "<h2>Post</h2>\n<p>Words.</p>\n<p class=\"post-date\">Created <time datetime=\"2026-01-18\">"
        ));
        assert!(!render_page_body(&page).contains("Last modified"));
        page.date_modified = Some("2026-09-29".into());
        assert!(render_page_body(&page).ends_with(
            "<br>Last modified <time datetime=\"2026-09-29\">September 29, 2026</time></p>"
        ));
        let mut pages = vec![page];
        assert!(blog_posts(&pages).is_ok());
        assert!(render_rss(&blog_posts(&pages).unwrap()).contains("Sun, 18 Jan 2026 00:00:00 GMT"));
        pages[0].date_modified = Some("2026-02-30".into());
        assert!(blog_posts(&pages).unwrap_err().contains("valid date"));
        pages[0].date_modified = Some("2025-01-01".into());
        assert!(blog_posts(&pages)
            .unwrap_err()
            .contains("before its creation date"));
    }
}
