//! Templates and components: their admin handlers, and the validation that
//! checks pages against them.
use std::collections::{HashMap, HashSet};

use axum::{
    Extension,
    extract::{Path as AxumPath, State},
    http::StatusCode,
};
use mysql::prelude::Queryable;
use serde::Deserialize;

use crate::{
    ApiError, AppState, Block, Component, ComponentId, Field, FieldKind, Json, Page, Region,
    Renderer, Result, Template, TemplateId, auth,
    db::{Locked, db_error, json, load_all, require_component, require_template},
    media, richtext,
};

/// Which kind of definition a deletion targets.
#[derive(Clone, Copy)]
pub(crate) enum DefinitionKind {
    Template,
    Component,
}

impl DefinitionKind {
    fn table(self) -> &'static str {
        match self {
            Self::Template => "templates",
            Self::Component => "components",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Template => "template",
            Self::Component => "component",
        }
    }
}

/// A template or component about to replace its stored version.
#[derive(Clone, Copy)]
pub(crate) enum SchemaChange<'a> {
    Template(&'a Template),
    Component(&'a Component),
}

#[derive(Deserialize)]
pub(crate) struct NewTemplate {
    name: String,
    description: String,
    regions: Vec<Region>,
}

pub(crate) async fn create_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewTemplate>,
) -> Result<(StatusCode, Json<Template>)> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let item = Template {
                id: TemplateId::new(),
                name: input.name,
                description: input.description,
                regions: input.regions,
            };
            validate_template(db, &item)?;
            db.exec_drop(
                "INSERT INTO templates(id,data) VALUES(?,?)",
                (&item.id, json(&item)?),
            )
            .map_err(db_error)?;
            Ok((StatusCode::CREATED, Json(item)))
        })
        .await
}

pub(crate) async fn update_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Template>,
) -> Result<Json<Template>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            if item.id != id.as_str() {
                return Err(ApiError::bad("template id does not match path"));
            }
            require_template(db, &item.id)?;
            validate_template(db, &item)?;
            validate_schema_change(db, SchemaChange::Template(&item))?;
            db.exec_drop("UPDATE templates SET data=? WHERE id=?", (json(&item)?, id))
                .map_err(db_error)?;
            Ok(Json(item))
        })
        .await
}

pub(crate) async fn delete_template(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            delete_definition(db, DefinitionKind::Template, &id)?;
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}

/// Delete an unused template or component. Published snapshots keep their own copies.
pub(crate) fn delete_definition(db: &mut Locked<'_>, kind: DefinitionKind, id: &str) -> Result<()> {
    let pages: Vec<Page> = load_all(db, "pages")?;
    let mut users = Vec::new();
    match kind {
        DefinitionKind::Template => users.extend(
            pages
                .iter()
                .filter(|p| p.template_id == id)
                .map(|p| format!("page {}", p.slug)),
        ),
        DefinitionKind::Component => {
            let templates: Vec<Template> = load_all(db, "templates")?;
            users.extend(
                templates
                    .iter()
                    .filter(|t| {
                        t.regions
                            .iter()
                            .any(|r| r.allowed_components.iter().any(|c| c == id))
                    })
                    .map(|t| format!("template {}", t.name)),
            );
            users.extend(
                pages
                    .iter()
                    .filter(|p| p.blocks.iter().any(|b| b.component_id == id))
                    .map(|p| format!("page {}", p.slug)),
            );
        }
    }
    if !users.is_empty() {
        return Err(ApiError::conflict(format!(
            "{} {id} is still used by {}",
            kind.name(),
            users.join(", ")
        )));
    }
    db.exec_drop(format!("DELETE FROM {} WHERE id=?", kind.table()), (id,))
        .map_err(db_error)?;
    if db.affected_rows() == 0 {
        return Err(ApiError::not_found());
    }
    Ok(())
}

#[derive(Deserialize)]
pub(crate) struct NewComponent {
    name: String,
    description: String,
    renderer: Renderer,
    fields: Vec<Field>,
}

pub(crate) async fn create_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(input): Json<NewComponent>,
) -> Result<(StatusCode, Json<Component>)> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let item = Component {
                id: ComponentId::new(),
                name: input.name,
                description: input.description,
                renderer: input.renderer,
                fields: input.fields,
            };
            validate_component(&item)?;
            db.exec_drop(
                "INSERT INTO components(id,data) VALUES(?,?)",
                (&item.id, json(&item)?),
            )
            .map_err(db_error)?;
            Ok((StatusCode::CREATED, Json(item)))
        })
        .await
}

pub(crate) async fn update_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(item): Json<Component>,
) -> Result<Json<Component>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            if item.id != id.as_str() {
                return Err(ApiError::bad("component id does not match path"));
            }
            require_component(db, &item.id)?;
            validate_component(&item)?;
            validate_schema_change(db, SchemaChange::Component(&item))?;
            db.exec_drop(
                "UPDATE components SET data=? WHERE id=?",
                (json(&item)?, id),
            )
            .map_err(db_error)?;
            Ok(Json(item))
        })
        .await
}

pub(crate) async fn delete_component(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            delete_definition(db, DefinitionKind::Component, &id)?;
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}

pub(crate) fn validate_schema_change(
    db: &mut impl Queryable,
    change: SchemaChange<'_>,
) -> Result<()> {
    let mut pages: Vec<Page> = load_all(db, "pages")?;
    pages.retain(|page| match change {
        SchemaChange::Template(t) => t.id == page.template_id,
        SchemaChange::Component(c) => page.blocks.iter().any(|b| b.component_id == c.id),
    });
    let mut definitions = Definitions::load(db, &pages)?;
    match change {
        SchemaChange::Template(t) => {
            definitions.templates.insert(t.id.clone(), t.clone());
        }
        SchemaChange::Component(c) => {
            definitions.components.insert(c.id.clone(), c.clone());
        }
    }
    definitions.load_media(db, &pages)?;
    for page in &pages {
        definitions.validate(page)?;
    }
    Ok(())
}

// One operation's definitions, shared by validation and snapshot construction.
// Never retained across transactions, so schema changes need no cache invalidation.
pub(crate) struct Definitions {
    pub(crate) templates: HashMap<TemplateId, Template>,
    pub(crate) components: HashMap<ComponentId, Component>,
    /// Existing media that the pages' media fields reference. Missing IDs are absent.
    pub(crate) media: HashMap<media::MediaId, media::Media>,
}

impl Definitions {
    pub(crate) fn load(db: &mut impl Queryable, pages: &[Page]) -> Result<Self> {
        let mut templates = HashMap::new();
        let mut components = HashMap::new();
        for page in pages {
            if !templates.contains_key(&page.template_id) {
                templates.insert(
                    page.template_id.clone(),
                    require_template(db, &page.template_id)?,
                );
            }
            for block in &page.blocks {
                if !components.contains_key(&block.component_id) {
                    components.insert(
                        block.component_id.clone(),
                        require_component(db, &block.component_id)?,
                    );
                }
            }
        }
        let mut definitions = Self {
            templates,
            components,
            media: HashMap::new(),
        };
        definitions.load_media(db, pages)?;
        Ok(definitions)
    }

    /// Loads media referenced under the current component definitions.
    pub(crate) fn load_media(&mut self, db: &mut impl Queryable, pages: &[Page]) -> Result<()> {
        for page in pages {
            for id in media::referenced(page, &self.components) {
                if !self.media.contains_key(&id)
                    && let Some(item) = media::find(db, &id)?
                {
                    self.media.insert(id, item);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn validate(&self, page: &Page) -> Result<()> {
        if page.title.trim().is_empty() {
            return Err(ApiError::bad("title is required"));
        }
        let mut aliases = HashSet::new();
        for alias in &page.aliases {
            if alias == &page.slug || !aliases.insert(alias) {
                return Err(ApiError::bad(
                    "aliases must be unique and different from the page path",
                ));
            }
        }
        let template = self
            .templates
            .get(&page.template_id)
            .ok_or_else(ApiError::not_found)?;
        let mut block_ids = HashSet::new();
        let mut counts: HashMap<&str, usize> = HashMap::new();
        for block in &page.blocks {
            if block.id.trim().is_empty() || !block_ids.insert(&block.id) {
                return Err(ApiError::bad("block ids must be non-empty and unique"));
            }
            let region = template
                .regions
                .iter()
                .find(|r| r.name == block.region)
                .ok_or_else(|| ApiError::bad(format!("unknown region: {}", block.region)))?;
            if !region.allowed_components.contains(&block.component_id) {
                return Err(ApiError::bad(format!(
                    "component {} is not allowed in region {}",
                    block.component_id, block.region
                )));
            }
            *counts.entry(&block.region).or_default() += 1;
            let component = self
                .components
                .get(&block.component_id)
                .ok_or_else(ApiError::not_found)?;
            validate_block(block, component, &self.media)?;
        }
        for region in &template.regions {
            if counts.get(region.name.as_str()).copied().unwrap_or(0) > region.max_components {
                return Err(ApiError::bad(format!(
                    "region {} exceeds maximum",
                    region.name
                )));
            }
        }
        Ok(())
    }
}

fn validate_block(
    block: &Block,
    component: &Component,
    media: &HashMap<media::MediaId, media::Media>,
) -> Result<()> {
    for key in block.fields.keys() {
        if !component.fields.iter().any(|f| &f.name == key) {
            return Err(ApiError::bad(format!("unknown field: {key}")));
        }
    }
    for field in &component.fields {
        let value = block
            .fields
            .get(&field.name)
            .map(|s| s.trim())
            .unwrap_or("");
        if field.required && value.is_empty() {
            return Err(ApiError::bad(format!("field {} is required", field.name)));
        }
        if matches!(field.kind, FieldKind::Url) && !value.is_empty() {
            validate_url(value)?;
        }
        if matches!(field.kind, FieldKind::Media)
            && !value.is_empty()
            && !media::MediaId::parse(value).is_some_and(|id| media.contains_key(&id))
        {
            return Err(ApiError::bad(format!(
                "field {} must reference uploaded media",
                field.name
            )));
        }
        if matches!(field.kind, FieldKind::Richtext) {
            let has_content = richtext::validate(value, field.richtext.as_ref())?;
            if field.required && !has_content {
                return Err(ApiError::bad(format!("field {} is required", field.name)));
            }
        }
    }
    Ok(())
}

pub(crate) fn validate_component(item: &Component) -> Result<()> {
    if item.name.trim().is_empty() {
        return Err(ApiError::bad("component name is required"));
    }
    let mut names = HashSet::new();
    for field in &item.fields {
        if field.name.trim().is_empty()
            || field.label.trim().is_empty()
            || !names.insert(&field.name)
        {
            return Err(ApiError::bad("field names must be non-empty and unique"));
        }
        if let Some(config) = &field.richtext {
            if !matches!(field.kind, FieldKind::Richtext) {
                return Err(ApiError::bad("richtext options require a richtext field"));
            }
            config.validate()?;
        }
    }
    Ok(())
}

pub(crate) fn validate_template(db: &mut impl Queryable, item: &Template) -> Result<()> {
    if item.name.trim().is_empty() {
        return Err(ApiError::bad("template name is required"));
    }
    let mut names = HashSet::new();
    for region in &item.regions {
        if region.name.trim().is_empty() || !names.insert(&region.name) {
            return Err(ApiError::bad("region names must be non-empty and unique"));
        }
        if region.max_components == 0 {
            return Err(ApiError::bad("region maximum must be positive"));
        }
        for id in &region.allowed_components {
            require_component(db, id)?;
        }
    }
    Ok(())
}

pub(crate) fn validate_url(value: &str) -> Result<()> {
    if value
        .chars()
        .any(|character| character.is_control() || character.is_whitespace() || character == '\\')
    {
        return Err(ApiError::bad(
            "URL must be a relative path or an http/https URL",
        ));
    }
    if value.starts_with('/') && !value.starts_with("//") {
        return Ok(());
    }
    let parsed = value.parse::<axum::http::Uri>().ok();
    if parsed.as_ref().is_some_and(|uri| {
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri
                .authority()
                .is_some_and(|authority| !authority.host().is_empty())
    }) {
        Ok(())
    } else {
        Err(ApiError::bad(
            "URL must be a relative path or an http/https URL",
        ))
    }
}
