use prettytable::{row, Table};

use crate::image::ImagesSummary;

/// writes the summary of images
pub fn write_summary(summary: &ImagesSummary, output: &mut impl std::io::Write) {
    let mut table = Table::new();

    table.add_row(row!["Repo", "Tags", "Total"]);

    for (repo, tags) in summary {
        let formatted_tags =
            tags.iter()
                .filter_map(|tag| tag.image_tag())
                .fold(String::new(), |mut acc, tag| {
                    acc.push_str(&format!("{}\n", tag));
                    acc
                });

        table.add_row(row![repo, formatted_tags, tags.len()]);
    }

    let _ = table.print(output);
}
