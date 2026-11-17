//! Reusable fuzz assertions, also exercised by deterministic regression tests.

pub mod bundle;
pub mod csv;
pub mod email_html;
pub mod landing;
pub mod mailbox;
pub mod routing;
pub mod submission;
pub mod template;

#[cfg(test)]
mod dependency_graph;

#[cfg(test)]
mod seeds {
    #[test]
    fn curated_inputs_replay_through_every_target() {
        type Target = (&'static str, fn(&[u8]));
        let targets: [Target; 9] = [
            ("submission_decode", super::submission::check),
            ("public_routing", super::routing::check),
            ("mailbox_identify", super::mailbox::check),
            ("template_render", super::template::check),
            ("csv_import", super::csv::check),
            ("page_bundle_raw", super::bundle::raw),
            ("page_bundle_structured", super::bundle::structured),
            ("email_html", super::email_html::check),
            ("landing_html", super::landing::check),
        ];
        for (target, check) in targets {
            let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("seeds")
                .join(target);
            let mut count = 0;
            for entry in std::fs::read_dir(directory).unwrap() {
                let path = entry.unwrap().path();
                if path.is_file() {
                    check(&std::fs::read(path).unwrap());
                    count += 1;
                }
            }
            assert!(count > 0, "missing seeds for {target}");
        }
    }
}
