use super::*;
use axum::Extension;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Admin,
    Reviewer,
    Editor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Member {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub paths: Vec<String>,
    pub groups: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub paths: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Organization {
    pub revision: i64,
    pub members: Vec<Member>,
    pub groups: Vec<Group>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Access {
    pub id: String,
    pub role: Role,
    pub paths: Vec<String>,
}

fn forbidden() -> ApiError {
    ApiError(StatusCode::FORBIDDEN, "permission denied".into())
}

impl Access {
    pub fn allows(&self, path: &str) -> bool {
        self.role != Role::Editor
            || self
                .paths
                .iter()
                .any(|scope| path == scope || is_descendant(path, scope))
    }
    pub(crate) fn page(&self, path: &str) -> Result<()> {
        self.allows(path).then_some(()).ok_or_else(forbidden)
    }
    pub(crate) fn admin(&self) -> Result<()> {
        (self.role == Role::Admin)
            .then_some(())
            .ok_or_else(forbidden)
    }
    pub(crate) fn reviewer(&self) -> Result<()> {
        (self.role != Role::Editor)
            .then_some(())
            .ok_or_else(forbidden)
    }
}

fn organization(db: &mut impl Queryable) -> Result<Organization> {
    load_one(db, "organization", "installation")
}

fn resolve(org: Organization, id: String, recovery: bool) -> Result<Access> {
    if recovery {
        return Ok(Access {
            id,
            role: Role::Admin,
            paths: vec![],
        });
    }
    let member = org
        .members
        .iter()
        .find(|m| m.id == id)
        .ok_or_else(forbidden)?;
    let mut paths = member.paths.clone();
    for group in org.groups.iter().filter(|g| member.groups.contains(&g.id)) {
        paths.extend(group.paths.clone());
    }
    Ok(Access {
        id,
        role: member.role.clone(),
        paths,
    })
}

impl AppState {
    pub(super) async fn run_as<T, F>(&self, editor: auth::Editor, f: F) -> Result<T>
    where
        T: Send + 'static,
        F: FnOnce(&mut Transaction<'_>, Access) -> Result<T> + Send + 'static,
    {
        let recovery = self.auth.recovery_admin(&editor);
        self.run(move |db| {
            let access = resolve(organization(db)?, editor.id, recovery)?;
            f(db, access)
        })
        .await
    }
}

pub(super) fn initialize(db: &mut impl Queryable) -> Result<()> {
    let mut org = Organization::default();
    if let Ok(id) = std::env::var("BADDIE_BOOTSTRAP_ADMIN_ID") {
        if id.trim().is_empty() {
            return Err(ApiError::bad("BADDIE_BOOTSTRAP_ADMIN_ID must not be empty"));
        }
        org.members.push(Member {
            id,
            name: "Bootstrap administrator".into(),
            role: Role::Admin,
            paths: vec![],
            groups: vec![],
        });
    }
    // Only applies when initializing membership for this installation. Never
    // resurrect a removed member or overwrite administrators' later changes.
    db.exec_drop(
        "INSERT IGNORE INTO organization(id,data) VALUES('installation',?)",
        (json(&org)?,),
    )
    .map_err(db_error)?;
    Ok(())
}

pub(super) async fn get_organization(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
) -> Result<Json<Organization>> {
    state
        .run_as(editor, |db, access| {
            access.admin()?;
            Ok(Json(organization(db)?))
        })
        .await
}

pub(super) async fn save_organization(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Json(mut org): Json<Organization>,
) -> Result<Json<Organization>> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            let old = organization(db)?;
            if org.revision != old.revision {
                return Err(ApiError::conflict("stale membership revision"));
            }
            let mut groups = HashSet::new();
            for group in &org.groups {
                if group.id.trim().is_empty()
                    || group.name.trim().is_empty()
                    || !groups.insert(&group.id)
                {
                    return Err(ApiError::bad(
                        "group IDs and names are required; IDs must be unique",
                    ));
                }
                for path in &group.paths {
                    validate_slug(path)?;
                }
            }
            let mut members = HashSet::new();
            for member in &org.members {
                if member.id.trim().is_empty()
                    || member.name.trim().is_empty()
                    || !members.insert(&member.id)
                {
                    return Err(ApiError::bad(
                        "member IDs and names are required; IDs must be unique",
                    ));
                }
                if member.groups.iter().any(|g| !groups.contains(g)) {
                    return Err(ApiError::bad("unknown group"));
                }
                for path in &member.paths {
                    validate_slug(path)?;
                }
            }
            if !org.members.iter().any(|m| m.role == Role::Admin)
                && old.members.iter().any(|m| m.role == Role::Admin)
            {
                return Err(ApiError::bad("keep at least one administrator"));
            }
            org.revision += 1;
            db.exec_drop(
                "UPDATE organization SET data=? WHERE id='installation'",
                (json(&org)?,),
            )
            .map_err(db_error)?;
            Ok(Json(org))
        })
        .await
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReviewStatus {
    Submitted,
    ChangesRequested,
    Approved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Review {
    pub id: String,
    pub submission_id: String,
    pub content: Content,
    pub submitted_by: String,
    pub status: ReviewStatus,
    pub feedback: String,
    pub reviewed_by: Option<String>,
}

pub(super) fn current_content(db: &mut impl Queryable, page: Page) -> Result<Content> {
    let template = require_template(db, &page.template_id)?;
    let mut components = components_for_page(db, &page)?;
    components.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(Content {
        page,
        template,
        components,
    })
}

pub(super) async fn submit(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<Revision>,
) -> Result<Json<Review>> {
    state.run_as(editor, move |db, access| {
        let page = require_page(db, &id)?;
        access.page(&page.slug)?;
        if page.revision != input.revision { return Err(ApiError::conflict("stale revision")); }
        validate_page(db, &page)?;
        let review = Review { id: id.clone(), submission_id: Uuid::new_v4().to_string(), content: current_content(db, page)?, submitted_by: access.id, status: ReviewStatus::Submitted, feedback: String::new(), reviewed_by: None };
        db.exec_drop("INSERT INTO reviews(id,data) VALUES(?,?) ON DUPLICATE KEY UPDATE data=VALUES(data)", (&id, json(&review)?)).map_err(db_error)?;
        Ok(Json(review))
    }).await
}

#[derive(Deserialize)]
pub(super) struct Decision {
    revision: i64,
    submission_id: String,
    feedback: String,
    approve: bool,
}

pub(super) async fn decide(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<String>,
    Json(input): Json<Decision>,
) -> Result<Json<Review>> {
    state
        .run_as(editor, move |db, access| {
            access.reviewer()?;
            let mut review: Review = load_one(db, "reviews", &id)?;
            if review.status != ReviewStatus::Submitted
                || review.content.page.revision != input.revision
                || review.submission_id != input.submission_id
            {
                return Err(ApiError::conflict(
                    "submission is no longer pending at this revision",
                ));
            }
            let page = require_page(db, &id)?;
            let current = current_content(db, page)?;
            if current != review.content {
                return Err(ApiError::conflict(
                    "draft or schema changed; submit again before reviewing",
                ));
            }
            if input.approve {
                publish(db, current.page)?;
                review.status = ReviewStatus::Approved;
            } else {
                if input.feedback.trim().is_empty() {
                    return Err(ApiError::bad(
                        "feedback is required when requesting changes",
                    ));
                }
                review.status = ReviewStatus::ChangesRequested;
            }
            review.feedback = input.feedback;
            review.reviewed_by = Some(access.id);
            db.exec_drop(
                "UPDATE reviews SET data=? WHERE id=?",
                (json(&review)?, &id),
            )
            .map_err(db_error)?;
            Ok(Json(review))
        })
        .await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn group_and_individual_scopes_are_segment_aware_and_unknown_ids_are_denied() {
        let org = Organization {
            revision: 0,
            members: vec![Member {
                id: "user".into(),
                name: "Editor".into(),
                role: Role::Editor,
                paths: vec!["/news".into()],
                groups: vec!["team".into()],
            }],
            groups: vec![Group {
                id: "team".into(),
                name: "Team".into(),
                paths: vec!["/about/team".into()],
            }],
        };
        assert!(resolve(org.clone(), "shared-admin".into(), false).is_err());
        let access = resolve(org, "user".into(), false).unwrap();
        for path in ["/news", "/news/a", "/about/team", "/about/team/a"] {
            assert!(access.allows(path));
        }
        for path in ["/", "/newspaper", "/about", "/about/teams", "/News"] {
            assert!(!access.allows(path));
        }
        assert!(access.admin().is_err());
        assert!(access.reviewer().is_err());
    }
}
