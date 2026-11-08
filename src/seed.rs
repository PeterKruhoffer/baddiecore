//! Starter content for an empty database: four components, the homepage
//! template and an unpublished Home draft. Edit this to change what a new site
//! starts with.
use std::collections::HashMap;

use mysql::{Pool, prelude::Queryable};

use crate::{
    Block, Component, Field, FieldKind, Page, Region, Renderer, Result, SitePath, Template,
    db::{self, TransactionMode, db_error, json},
    serialization, store,
};

pub(crate) fn seed(db: &Pool) -> Result<()> {
    db::transaction(db, TransactionMode::Commit, |tx| {
        let count: i64 = tx
            .query_first("SELECT count(*) FROM components")
            .map_err(db_error)?
            .unwrap();
        if count != 0 {
            return Ok(());
        }
        let specs = [
            (
                "hero",
                "Hero",
                Renderer::Hero,
                vec![
                    ("eyebrow", false, FieldKind::Text),
                    ("title", true, FieldKind::Text),
                    ("body", false, FieldKind::Richtext),
                    ("button_label", false, FieldKind::Text),
                    ("button_url", false, FieldKind::Url),
                    ("image", false, FieldKind::Media),
                ],
            ),
            (
                "text",
                "Text",
                Renderer::Text,
                vec![
                    ("title", true, FieldKind::Text),
                    ("body", false, FieldKind::Richtext),
                ],
            ),
            (
                "callout",
                "Callout",
                Renderer::Callout,
                vec![
                    ("title", true, FieldKind::Text),
                    ("body", false, FieldKind::Richtext),
                    ("button_label", false, FieldKind::Text),
                    ("button_url", false, FieldKind::Url),
                ],
            ),
            (
                "cards",
                "Cards",
                Renderer::Cards,
                vec![
                    ("title", true, FieldKind::Text),
                    ("body", false, FieldKind::Textarea),
                ],
            ),
        ];
        let mut components = Vec::new();
        for (id, name, renderer, fields) in specs {
            let component = Component {
                id: id.parse().expect("seed component id"),
                name: name.into(),
                description: format!("{name} content block"),
                renderer,
                fields: fields
                    .into_iter()
                    .map(|(name, required, kind)| Field {
                        name: name.into(),
                        label: title_case(name),
                        kind,
                        required,
                        richtext: None,
                    })
                    .collect(),
            };
            tx.exec_drop(
                "INSERT INTO components(id,data) VALUES(?,?)",
                (&component.id, json(&component)?),
            )
            .map_err(db_error)?;
            components.push(component);
        }
        let template = Template {
            id: "homepage".parse().expect("seed template id"),
            name: "Homepage".into(),
            description: "A flexible homepage".into(),
            regions: vec![Region {
                name: "main".into(),
                allowed_components: components.iter().map(|c| c.id.clone()).collect(),
                max_components: 20,
            }],
        };
        tx.exec_drop(
            "INSERT INTO templates(id,data) VALUES(?,?)",
            (&template.id, json(&template)?),
        )
        .map_err(db_error)?;
        let blocks = vec![
            Block {
                id: "welcome-hero".into(),
                component_id: "hero".parse().expect("seed component id"),
                region: "main".into(),
                fields: HashMap::from([
                    ("eyebrow".into(), "Welcome".into()),
                    ("title".into(), "Build your site".into()),
                    (
                        "body".into(),
                        "Edit this starter page in the admin area.".into(),
                    ),
                ]),
                hidden: false,
            },
            Block {
                id: "welcome-text".into(),
                component_id: "text".parse().expect("seed component id"),
                region: "main".into(),
                fields: HashMap::from([
                    ("title".into(), "Start publishing".into()),
                    (
                        "body".into(),
                        "Add content, preview your draft, and publish when it is ready.".into(),
                    ),
                ]),
                hidden: false,
            },
        ];
        let page = Page {
            id: "home".parse().expect("seed page id"),
            title: "Home".into(),
            slug: SitePath::root(),
            aliases: vec![],
            template_id: template.id.clone(),
            blocks,
            revision: 1,
            published_revision: None,
        };
        store::save_pages(tx, &[page])?;
        serialization::record_base(tx, &components, &[template])
    })
}

fn title_case(value: &str) -> String {
    let mut chars = value.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().collect::<String>() + chars.as_str())
        .unwrap_or_default()
        .replace('_', " ")
}
