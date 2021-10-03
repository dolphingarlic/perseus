use crate::templates::docs::icons::{ERROR_ICON, WARNING_ICON};
use crate::templates::docs::template::DocsPageProps;
use lazy_static::lazy_static;
use perseus::{link, t, RenderFnResult, RenderFnResultWithCause};
use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use sycamore::prelude::Template as SycamoreTemplate;
use sycamore::prelude::*;
use walkdir::WalkDir;

pub fn parse_md_to_html(markdown: &str) -> String {
    let mut opts = Options::empty();
    // TODO possibly enable further features here if necessary
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    let parser = Parser::new_ext(markdown, opts);
    let mut html_contents = String::new();
    html::push_html(&mut html_contents, parser);

    html_contents
}

// By using a lazy static, we won't read from the filesystem in client-side code
lazy_static! {
    /// The latest version of the documentation. This will need to be updated as the docs are from the `docs/stable.txt` file.
    static ref DOCS_MANIFEST: DocsManifest = {
        let contents = fs::read_to_string("../../docs/manifest.json").unwrap();
        serde_json::from_str(&contents).unwrap()
    };
}

/// The stability of a version of the docs, which governs what kind of warning will be displayed.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum DocsVersionStatus {
    /// This version is stable, and no warning is needed.
    Stable,
    /// This version is outdated, and the latest stable version is attached.
    Outdated(String),
    /// This version is released, but in beta, and the latest stable version is attached.
    Beta(String),
    /// This documentation is for the unreleased next version, and the latest stable version is attached.
    Next(String),
}
impl DocsVersionStatus {
    /// Renders the docs status to a Sycamore template for display.
    pub fn render<G: GenericNode>(&self) -> SycamoreTemplate<G> {
        match &self {
            // No message should be displayed if it's the correct version
            Self::Stable => template! {},
            Self::Outdated(stable) => {
                let stable = stable.to_string();
                template! {
                    div(class = "ring-4 ring-red-400 p-4 rounded-lg mt-1") {
                        div(class = "flex flex-col 2xs:flex-row") {
                            span(
                                class = "self-center mr-2",
                                style = "fill: #f87171;",
                                dangerously_set_inner_html = ERROR_ICON
                            )
                            p(dangerously_set_inner_html = &t!("docs-status.outdated", {
                                "link": link!(&format!("/docs/{}/intro", stable))
                            }))
                        }
                    }
                }
            }
            Self::Beta(stable) => {
                let stable = stable.to_string();
                template! {
                    div(class = "ring-4 ring-yellow-300 p-4 rounded-lg mt-1") {
                        div(class = "flex flex-col 2xs:flex-row") {
                            span(
                                class = "self-center mr-2",
                                style = "fill: #fcd34d;",
                                dangerously_set_inner_html = WARNING_ICON
                            )
                            p(dangerously_set_inner_html = &t!("docs-status.beta", {
                                "link": link!(&format!("/docs/{}/intro", stable))
                            }))
                        }
                    }
                }
            }
            Self::Next(stable) => {
                let stable = stable.to_string();
                template! {
                    div(class = "ring-4 ring-orange-400 p-4 rounded-lg mt-1") {
                        div(class = "flex flex-col 2xs:flex-row") {
                            span(
                                class = "self-center mr-2",
                                style = "fill: #fb923c;",
                                dangerously_set_inner_html = ERROR_ICON
                            )
                            p(dangerously_set_inner_html = &t!("docs-status.next", {
                                "link": link!(&format!("/docs/{}/intro", stable))
                            }))
                        }
                    }
                }
            }
        }
    }
}
/// Information about the current state of the documentation, including which versions are outdated and the like.
#[derive(Serialize, Deserialize, Clone)]
pub struct DocsManifest {
    pub stable: String,
    pub outdated: Vec<String>,
    pub beta: Vec<String>,
}

pub async fn get_build_state(path: String, locale: String) -> RenderFnResultWithCause<String> {
    let path_vec: Vec<&str> = path.split('/').collect();
    // Localize the path again to what it'll be on the filesystem
    // TODO get Perseus to pass in props from build paths for ease of use?
    // We'll do that differently if it doesn't have a version in front of it, which would be the second part containing two dots
    // Or it could be `next`
    // If the path is just `/docs` though, we'll render the introduction page for the stable version
    let version: &str;
    let fs_path = if path == "docs" {
        version = &DOCS_MANIFEST.stable;
        format!(
            "{}/{}/{}/{}",
            path_vec[0], // `docs`
            &DOCS_MANIFEST.stable,
            &locale,
            "intro"
        )
    } else if path_vec[1].split('.').count() == 3 || path_vec[1] == "next" {
        version = path_vec[1];
        format!(
            "{}/{}/{}/{}",
            path_vec[0], // `docs`
            path_vec[1], // The version
            &locale,
            path_vec[2..].join("/") // The rest of the path
        )
    } else {
        version = &DOCS_MANIFEST.stable;
        // If it doesn't have a version, we'll inject the latest stable one
        format!(
            "{}/{}/{}/{}",
            path_vec[0], // `docs`
            &DOCS_MANIFEST.stable,
            &locale,
            path_vec[1..].join("/") // The rest of the path
        )
    };
    let fs_path = format!("../../{}.md", fs_path);
    // Read that file
    let contents = fs::read_to_string(&fs_path)?;
    let html_contents = parse_md_to_html(&contents);
    // Get the title from the first line of the contents, stripping the initial `#`
    // This is brittle, but surprisingly quite reliable as long as documentation files have headings
    dbg!(&path);
    let title = contents.lines().collect::<Vec<&str>>()[0]
        .strip_prefix("# ")
        .unwrap();

    // Get the sidebar from `SUMMARY.md`
    let sidebar_fs_path = format!("../../docs/{}/{}/SUMMARY.md", &version, &locale);
    let sidebar_contents = fs::read_to_string(&sidebar_fs_path)?;
    // Replace all links in that file with localized equivalents with versions as well
    // That means unversioned paths will redirect to the appropriate stable version
    let sidebar_contents =
        sidebar_contents.replace("/docs", &format!("/{}/docs/{}", &locale, &version));
    let sidebar_html_contents = parse_md_to_html(&sidebar_contents);

    // Work out the status of this page
    let status = if version == "next" {
        DocsVersionStatus::Next(DOCS_MANIFEST.stable.to_string())
    } else if DOCS_MANIFEST.outdated.contains(&version.to_string()) {
        DocsVersionStatus::Outdated(DOCS_MANIFEST.stable.to_string())
    } else if DOCS_MANIFEST.beta.contains(&version.to_string()) {
        DocsVersionStatus::Beta(DOCS_MANIFEST.stable.to_string())
    } else if DOCS_MANIFEST.stable == version {
        DocsVersionStatus::Stable
    } else {
        unreachable!()
    };

    let props = DocsPageProps {
        title: title.to_string(),
        content: html_contents,
        sidebar_content: sidebar_html_contents,
        status,
        manifest: DOCS_MANIFEST.clone(),
        current_version: version.to_string(),
    };

    let props_str = serde_json::to_string(&props)?;
    Ok(props_str)
}

pub async fn get_build_paths() -> RenderFnResult<Vec<String>> {
    // We start off by rendering the `/docs` page itself as an alias
    let mut paths: Vec<String> = vec!["".to_string()];
    // Get the `docs/` directory (relative to `.perseus/`)
    let docs_dir = PathBuf::from("../../docs");
    // Loop through it
    for entry in WalkDir::new(docs_dir) {
        let entry = entry?;
        let path = entry.path();
        // Ignore any empty directories or the like
        if path.is_file() {
            // This should all pass, there are no non-Unicode filenames in the docs (and i18n titles are handled outside filenames)
            // Also, all these are relative, which means we can safely strip away the `../../docs/`
            // We also remove the file extensions (which are all `.md`)
            let path_str = path.to_str().unwrap().replace(".md", "");
            let path_str = path_str.strip_prefix("../../docs/").unwrap();
            // Only proceed for paths in the default locale (`en-US`), which we'll use to generate paths
            // Also disallow any of the `SUMMARY.md` files at this point (the extension has been stripped)
            // Also disallow the manifest file
            if path_str.contains("en-US/")
                && !path_str.ends_with("SUMMARY")
                && !path_str.ends_with("manifest.json")
            {
                // Now remove that locale (it'll be put at the front of the path in the URL)
                let path_str = path_str.replace("en-US/", "");
                // This path should be rendered!
                paths.push(path_str.clone());
                // If it's for the latest stable version though, we should also render it without that prefix
                // That way the latest stable verison is always at the docs without a version prefix (which I think is more sensible than having the unreleased version there)
                if path_str.starts_with(&DOCS_MANIFEST.stable) {
                    let unprefixed_path_str = path_str
                        .strip_prefix(&format!("{}/", &DOCS_MANIFEST.stable))
                        .unwrap();
                    paths.push(unprefixed_path_str.to_string());
                }
            }
        }
    }

    Ok(paths)
}