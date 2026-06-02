//! Internal plumbing shared by the async and blocking client surfaces:
//! the request executors and small pure helpers used by the generated
//! method implementations.

use icloud_protocol::{
    RequestSpec, ResponseParts, header, parse_challenge_headers, parse_session_seed,
};
use icloud_types::{
    AccountMemberPhoto, DriveNode, FamilyMember, FindMyDevice, FindMyDevicesResponse, ICloudError,
    PendingTwoFactorAuth, PhotoAlbumSummary, PhotoLibraryState,
};

use crate::{AsyncTransport, BlockingTransport, ClientCore, ClientError};

pub(crate) async fn execute_async<T, S>(
    core: &mut ClientCore<T, S>,
    mut request: RequestSpec,
) -> Result<ResponseParts, ClientError<T::Error>>
where
    T: AsyncTransport,
{
    core.prepare_request(&mut request)?;
    let request_url = request.url.clone();
    let response = core
        .transport
        .execute(request)
        .await
        .map_err(ClientError::Transport)?;
    core.ingest_response(&request_url, &response)?;
    Ok(response)
}

pub(crate) fn execute_blocking<T, S>(
    core: &mut ClientCore<T, S>,
    mut request: RequestSpec,
) -> Result<ResponseParts, ClientError<T::Error>>
where
    T: BlockingTransport,
{
    core.prepare_request(&mut request)?;
    let request_url = request.url.clone();
    let response = core
        .transport
        .execute(request)
        .map_err(ClientError::Transport)?;
    core.ingest_response(&request_url, &response)?;
    Ok(response)
}

pub(crate) fn merge_pending_two_factor_response(
    pending: &mut PendingTwoFactorAuth,
    response: &ResponseParts,
) -> Result<(), ICloudError> {
    pending.seed.merge(parse_session_seed(response)?);
    if let Some(challenge) = parse_challenge_headers(response)? {
        pending.challenge = challenge;
    }
    Ok(())
}

pub(crate) fn require_etag(node: &DriveNode) -> Result<&str, ICloudError> {
    node.metadata()
        .etag
        .as_deref()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "drive node is missing an `etag`".to_owned(),
        })
}

pub(crate) fn findmy_device_or_error(
    response: FindMyDevicesResponse,
    device_id: &str,
) -> Result<FindMyDevice, ICloudError> {
    response
        .into_device(device_id)
        .ok_or_else(|| ICloudError::FindMyDeviceNotFound {
            id: device_id.to_owned(),
        })
}

pub(crate) fn merge_photo_albums(custom_albums: Vec<PhotoAlbumSummary>) -> Vec<PhotoAlbumSummary> {
    let mut albums = PhotoAlbumSummary::smart_albums();
    albums.extend(custom_albums);
    albums
}

pub(crate) fn photo_library_not_ready(state: PhotoLibraryState) -> ICloudError {
    ICloudError::PhotosLibraryNotReady {
        state: state.as_str().to_owned(),
    }
}

pub(crate) fn family_member_id(member: &FamilyMember) -> Result<&str, ICloudError> {
    member
        .dsid
        .as_deref()
        .ok_or_else(|| ICloudError::InvalidResponse {
            message: "family member is missing `dsid`".to_owned(),
        })
}

pub(crate) fn account_member_photo(member_id: &str, response: ResponseParts) -> AccountMemberPhoto {
    let content_type = response
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned);
    AccountMemberPhoto {
        member_id: member_id.to_owned(),
        body: response.body,
        content_type,
    }
}

pub(crate) fn unix_timestamp_ms() -> Result<i64, ICloudError> {
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| ICloudError::InvalidResponse {
            message: format!("system clock is earlier than the unix epoch: {error}"),
        })?;
    duration
        .as_millis()
        .try_into()
        .map_err(|error| ICloudError::InvalidResponse {
            message: format!("current unix timestamp does not fit in i64: {error}"),
        })
}
