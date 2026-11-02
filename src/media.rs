//! Uploaded images. Bytes live in MySQL beside their metadata and never change
//! after upload, so public responses are cacheable forever.
use std::collections::BTreeSet;

use axum::body::Bytes;

use super::*;

/// Largest accepted upload. The upload route has its own body limit.
pub(crate) const MAX_BYTES: usize = 10 * 1024 * 1024;

/// Identifies an uploaded media item. The server creates IDs; incoming and
/// stored values become a `MediaId` only through `parse` or deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(transparent)]
pub struct MediaId(String);

impl MediaId {
    fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Accepts only the canonical hyphenated UUID form the server issues.
    pub fn parse(value: &str) -> Option<Self> {
        Uuid::try_parse(value)
            .ok()
            .filter(|id| id.to_string() == value)
            .map(|_| Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Media {
    pub id: MediaId,
    pub filename: String,
    pub content_type: String,
    pub size: i64,
    pub alt: String,
    pub uploaded_by: String,
    /// Unix seconds.
    pub created_at: i64,
}

/// The content type of a supported image, judged by its leading bytes only.
fn sniff(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("image/png")
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some("image/jpeg")
    } else if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") {
        Some("image/gif")
    } else if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("image/webp")
    } else if is_avif(bytes) {
        Some("image/avif")
    } else {
        None
    }
}

/// AVIF files open with an ISO-BMFF `ftyp` box listing an AVIF brand.
fn is_avif(bytes: &[u8]) -> bool {
    if bytes.len() < 16 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) as usize;
    let header = &bytes[..size.clamp(16, bytes.len())];
    // Major brand at 8..12, minor version at 12..16, compatible brands after.
    let major = header[8..12].as_chunks::<4>().0;
    let (compatible, _) = header[16..].as_chunks::<4>();
    major
        .iter()
        .chain(compatible)
        .any(|brand| brand == b"avif" || brand == b"avis")
}

/// Media IDs that `page`'s blocks reference through media fields. Values that
/// are not media IDs are skipped; validation reports them.
pub(crate) fn referenced<'a>(
    page: &'a Page,
    components: &'a HashMap<String, Component>,
) -> impl Iterator<Item = MediaId> + 'a {
    page.blocks.iter().flat_map(move |block| {
        components
            .get(&block.component_id)
            .into_iter()
            .flat_map(|component| &component.fields)
            .filter(|field| matches!(field.kind, FieldKind::Media))
            .filter_map(|field| block.fields.get(&field.name))
            .filter_map(|value| MediaId::parse(value))
    })
}

pub(crate) fn find(db: &mut impl Queryable, id: &MediaId) -> Result<Option<Media>> {
    let raw: Option<String> = db
        .exec_first("SELECT data FROM media WHERE id=?", (id.as_str(),))
        .map_err(db_error)?;
    raw.map(|raw| parse(&raw)).transpose()
}

/// Every media item, newest first, without bytes.
pub(crate) fn list(db: &mut impl Queryable) -> Result<Vec<Media>> {
    let mut media: Vec<Media> = load_all(db, "media")?;
    media.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
    Ok(media)
}

#[derive(Deserialize)]
pub(super) struct Upload {
    filename: String,
}

/// Any member may upload. The client's Content-Type header is ignored.
pub(super) async fn upload(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    Query(input): Query<Upload>,
    bytes: Bytes,
) -> Result<(StatusCode, Json<Media>)> {
    let filename = input.filename.trim().to_owned();
    if filename.is_empty() || filename.chars().count() > 255 {
        return Err(ApiError::bad("filename must have 1 to 255 characters"));
    }
    let content_type = sniff(&bytes)
        .ok_or_else(|| ApiError::bad("only PNG, JPEG, GIF, WebP and AVIF images are supported"))?;
    state
        .run_as(editor, move |db, access| {
            let media = Media {
                id: MediaId::new(),
                filename,
                content_type: content_type.into(),
                size: bytes.len() as i64,
                alt: String::new(),
                uploaded_by: access.id,
                created_at: schedule::now(),
            };
            db.exec_drop(
                "INSERT INTO media(id,data,bytes) VALUES(?,?,?)",
                (media.id.as_str(), json(&media)?, bytes.as_ref()),
            )
            .map_err(db_error)?;
            Ok((StatusCode::CREATED, Json(media)))
        })
        .await
}

#[derive(Deserialize)]
pub(super) struct Alt {
    alt: String,
}

pub(super) async fn update(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<MediaId>,
    Json(input): Json<Alt>,
) -> Result<Json<Media>> {
    state
        .run_as(editor, move |db, _| {
            let mut media = find(db, &id)?.ok_or_else(ApiError::not_found)?;
            media.alt = input.alt;
            db.exec_drop(
                "UPDATE media SET data=? WHERE id=?",
                (json(&media)?, id.as_str()),
            )
            .map_err(db_error)?;
            Ok(Json(media))
        })
        .await
}

/// Admin only. Refuses while any draft, live page, schedule or review uses the item.
pub(super) async fn delete(
    State(state): State<AppState>,
    Extension(editor): Extension<auth::Editor>,
    AxumPath(id): AxumPath<MediaId>,
) -> Result<StatusCode> {
    state
        .run_as(editor, move |db, access| {
            access.admin()?;
            find(db, &id)?.ok_or_else(ApiError::not_found)?;
            let users = users(db, &id)?;
            if !users.is_empty() {
                return Err(ApiError::conflict(format!(
                    "media is still used by {}",
                    users.into_iter().collect::<Vec<_>>().join(", ")
                )));
            }
            db.exec_drop("DELETE FROM media WHERE id=?", (id.as_str(),))
                .map_err(db_error)?;
            Ok(StatusCode::NO_CONTENT)
        })
        .await
}

/// Paths of pages whose draft, published snapshot, scheduled snapshot or review uses `id`.
fn users(db: &mut impl Queryable, id: &MediaId) -> Result<BTreeSet<String>> {
    let components: HashMap<String, Component> = load_all::<Component>(db, "components")?
        .into_iter()
        .map(|component| (component.id.clone(), component))
        .collect();
    let mut users = BTreeSet::new();
    for page in load_all::<Page>(db, "pages")? {
        if referenced(&page, &components).any(|used| &used == id) {
            users.insert(page.slug);
        }
    }
    let snapshots: Vec<String> = db
        .query("SELECT data FROM snapshots UNION ALL SELECT data FROM scheduled_publications")
        .map_err(db_error)?;
    let mut contents = snapshots
        .iter()
        .map(|raw| parse::<Content>(raw))
        .collect::<Result<Vec<_>>>()?;
    contents.extend(
        load_all::<workflow::Review>(db, "reviews")?
            .into_iter()
            .map(|review| review.content),
    );
    for content in contents {
        if content.media.iter().any(|media| &media.id == id) {
            users.insert(content.page.slug);
        }
    }
    Ok(users)
}

/// Public and lock-free, like published content.
pub(super) async fn serve(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<MediaId>,
) -> Result<Response> {
    let (data, bytes) = state
        .read(move |db| {
            db.exec_first::<(String, Vec<u8>), _, _>(
                "SELECT data, bytes FROM media WHERE id=?",
                (id.as_str(),),
            )
            .map_err(db_error)?
            .ok_or_else(ApiError::not_found)
        })
        .await?;
    let media: Media = parse(&data)?;
    Ok((
        [
            (header::CONTENT_TYPE, media.content_type),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_owned(),
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_owned()),
        ],
        bytes,
    )
        .into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sniffs_supported_images_and_rejects_everything_else() {
        let avif = b"\0\0\0\x1cftypmif1\0\0\0\0mif1avifmiaf";
        for (bytes, expected) in [
            (&b"\x89PNG\r\n\x1a\n...."[..], Some("image/png")),
            (b"\xff\xd8\xff\xe0", Some("image/jpeg")),
            (b"GIF89a....", Some("image/gif")),
            (b"RIFF\0\0\0\0WEBPVP8 ", Some("image/webp")),
            (avif, Some("image/avif")),
            (b"\0\0\0\x18ftypmp42\0\0\0\0isommp42", None),
            (b"<html><script>", None),
            (b"<svg xmlns=", None),
            (b"", None),
        ] {
            assert_eq!(sniff(bytes), expected, "{bytes:?}");
        }
    }

    #[test]
    fn media_ids_are_canonical_uuids() {
        let id = MediaId::new();
        assert_eq!(MediaId::parse(id.as_str()), Some(id.clone()));
        assert_eq!(MediaId::parse(&id.as_str().to_uppercase()), None);
        assert_eq!(MediaId::parse(""), None);
        assert_eq!(MediaId::parse("logo.png"), None);
    }
}
