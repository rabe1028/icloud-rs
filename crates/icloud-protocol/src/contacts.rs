use crate::prelude::*;

use crate::Method;
use crate::json::{Map, Value};
use url::Url;

use icloud_types::{
    ContactCard, ContactEmail, ContactPhone, ContactsPage, ContactsStartup, ICloudError,
};

use crate::{RequestSpec, ResponseParts};

#[derive(Debug, Clone)]
pub struct ContactsProtocol<'a> {
    service_root: &'a Url,
    client_id: &'a str,
    dsid: &'a str,
}

impl<'a> ContactsProtocol<'a> {
    #[must_use]
    pub fn new(service_root: &'a Url, client_id: &'a str, dsid: &'a str) -> Self {
        Self {
            service_root,
            client_id,
            dsid,
        }
    }

    pub fn startup(&self) -> Result<RequestSpec, ICloudError> {
        self.request("co/startup", core::iter::empty::<(&str, String)>())
    }

    pub fn contacts(
        &self,
        pref_token: &str,
        sync_token: &str,
        limit: usize,
        offset: usize,
    ) -> Result<RequestSpec, ICloudError> {
        self.request(
            "co/contacts",
            [
                ("prefToken", pref_token.to_owned()),
                ("syncToken", sync_token.to_owned()),
                ("limit", limit.to_string()),
                ("offset", offset.to_string()),
            ],
        )
    }

    fn request<I>(&self, path: &str, extra: I) -> Result<RequestSpec, ICloudError>
    where
        I: IntoIterator<Item = (&'static str, String)>,
    {
        let mut url = self.service_root.join(path)?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("clientId", self.client_id);
            query.append_pair("dsid", self.dsid);
            query.append_pair("clientVersion", "2.1");
            query.append_pair("locale", "en_US");
            query.append_pair("order", "last,first");
            query.append_pair("includePhoneNumbers", "true");
            for (key, value) in extra {
                query.append_pair(key, value.as_str());
            }
        }
        Ok(RequestSpec::new(Method::Get, url))
    }
}

pub fn parse_contacts_startup(response: &ResponseParts) -> Result<ContactsStartup, ICloudError> {
    let payload = crate::json::object(response)?;
    Ok(ContactsStartup {
        pref_token: crate::json::string_field(&payload, "prefToken").unwrap_or_default(),
        sync_token: crate::json::string_field(&payload, "syncToken").unwrap_or_default(),
        extra: payload,
    })
}

pub fn parse_contacts_page(response: &ResponseParts) -> Result<ContactsPage, ICloudError> {
    let payload = crate::json::object(response)?;
    let contacts = payload
        .get("contacts")
        .and_then(Value::as_array)
        .unwrap_or_default()
        .iter()
        .cloned()
        .filter_map(|value| match value {
            Value::Object(value) => Some(value),
            _ => None,
        })
        .map(parse_contact_card)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ContactsPage {
        contacts,
        sync_token: crate::json::string_field(&payload, "syncToken"),
        extra: payload,
    })
}

fn parse_contact_card(raw: Map<String, Value>) -> Result<ContactCard, ICloudError> {
    let id = string_field_any(&raw, &["contactId", "identifier", "recordName", "id"]).ok_or_else(
        || ICloudError::InvalidResponse {
            message: "contact payload is missing an identifier".to_owned(),
        },
    )?;

    Ok(ContactCard {
        id,
        first_name: string_field_any(&raw, &["firstName", "first"]),
        last_name: string_field_any(&raw, &["lastName", "last"]),
        display_name: string_field_any(&raw, &["displayName", "fullName"]),
        middle_name: string_field_any(&raw, &["middleName"]),
        prefix: string_field_any(&raw, &["prefix"]),
        suffix: string_field_any(&raw, &["suffix"]),
        nickname: string_field_any(&raw, &["nickname"]),
        company_name: string_field_any(&raw, &["companyName", "organization"]),
        emails: parse_contact_details(
            &raw,
            &["emails", "emailAddresses"],
            &["value", "address"],
            parse_contact_email,
        )?,
        phones: parse_contact_details(
            &raw,
            &["phones", "phoneNumbers"],
            &["value", "number"],
            parse_contact_phone,
        )?,
        raw,
    })
}

fn parse_contact_email(raw: Map<String, Value>, address: String) -> ContactEmail {
    ContactEmail {
        label: string_field_any(&raw, &["label", "type"]),
        address,
        extra: raw,
    }
}

fn parse_contact_phone(raw: Map<String, Value>, number: String) -> ContactPhone {
    ContactPhone {
        label: string_field_any(&raw, &["label", "type"]),
        number,
        extra: raw,
    }
}

fn parse_contact_details<T, F>(
    raw: &Map<String, Value>,
    collection_keys: &[&str],
    value_keys: &[&str],
    build: F,
) -> Result<Vec<T>, ICloudError>
where
    F: Fn(Map<String, Value>, String) -> T,
{
    let Some(entries) = array_field_any(raw, collection_keys) else {
        return Ok(Vec::new());
    };

    let mut parsed = Vec::new();
    for entry in entries {
        let object = entry
            .as_object()
            .ok_or_else(|| ICloudError::InvalidResponse {
                message: "contact detail entry is not an object".to_owned(),
            })?;
        let Some(value) = string_field_any(object, value_keys) else {
            continue;
        };
        parsed.push(build(object.clone(), value));
    }
    Ok(parsed)
}

fn string_field_any(raw: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| match raw.get(*key) {
        Some(Value::String(value)) => Some(value.clone()),
        _ => None,
    })
}

fn array_field_any<'a>(raw: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Vec<Value>> {
    keys.iter().find_map(|key| match raw.get(*key) {
        Some(Value::Array(values)) => Some(values),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use crate::{HeaderMap, StatusCode};
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn startup_request_uses_contacts_refresh_query_params() {
        let root = Url::parse("https://p43-contacts.icloud.com").expect("valid url");
        let protocol = ContactsProtocol::new(&root, "client-123", "42");

        let request = protocol.startup().expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-contacts.icloud.com/co/startup?clientId=client-123&dsid=42&clientVersion=2.1&locale=en_US&order=last%2Cfirst&includePhoneNumbers=true",
        );
    }

    #[test]
    fn contacts_request_includes_pref_sync_limit_and_offset() {
        let root = Url::parse("https://p43-contacts.icloud.com").expect("valid url");
        let protocol = ContactsProtocol::new(&root, "client-123", "42");

        let request = protocol
            .contacts("pref-token", "sync-token", 0, 0)
            .expect("request should build");

        assert_eq!(
            request.url.as_str(),
            "https://p43-contacts.icloud.com/co/contacts?clientId=client-123&dsid=42&clientVersion=2.1&locale=en_US&order=last%2Cfirst&includePhoneNumbers=true&prefToken=pref-token&syncToken=sync-token&limit=0&offset=0",
        );
    }

    #[test]
    fn parser_extracts_contacts_startup_tokens() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/contacts/startup.json").to_vec(),
        );

        let startup = parse_contacts_startup(&response).expect("startup should parse");

        assert_eq!(startup.pref_token, "pref-token-1");
        assert_eq!(startup.sync_token, "sync-token-1");
    }

    #[test]
    fn parser_extracts_contacts_with_emails_and_phones() {
        let response = ResponseParts::new(
            StatusCode::OK,
            HeaderMap::new(),
            include_bytes!("../tests/fixtures/contacts/contacts.json").to_vec(),
        );

        let page = parse_contacts_page(&response).expect("contacts should parse");

        assert_eq!(page.contacts.len(), 2);
        assert_eq!(page.contacts[0].id, "contact-1");
        assert_eq!(page.contacts[0].emails[0].address, "ada@example.com");
        assert_eq!(page.contacts[1].id, "contact-2");
        assert_eq!(page.contacts[1].phones[0].number, "+1-555-0100");
    }
}
