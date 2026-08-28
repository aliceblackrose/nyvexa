use std::{collections::HashSet, time::Duration};

use reqwest::Client;
use scraper::{ElementRef, Html, Selector};
use thiserror::Error;
use url::Url;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CharacterSearchResult {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) world: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CharacterProfile {
    pub(crate) biography: String,
    pub(crate) free_company_id: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FreeCompanyMember {
    pub(crate) character_id: u64,
    pub(crate) name: String,
    pub(crate) world: String,
    pub(crate) rank: String,
}

#[derive(Debug, Error)]
pub(crate) enum LodestoneError {
    #[error("Lodestone request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("could not parse Lodestone HTML: {0}")]
    Parse(String),
    #[error("character not found")]
    CharacterNotFound,
    #[error("multiple exact character matches were returned")]
    AmbiguousCharacter,
}

#[derive(Clone)]
pub(crate) struct LodestoneClient {
    http: Client,
    base_url: Url,
}

impl LodestoneClient {
    pub(crate) fn new(region: &str) -> Result<Self, LodestoneError> {
        let base_url = Url::parse(&format!("https://{region}.finalfantasyxiv.com/"))
            .map_err(|error| LodestoneError::Parse(error.to_string()))?;
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent("Nyvexa/0.1 (+https://github.com/aliceblackrose/nyvexa)")
            .build()?;
        Ok(Self { http, base_url })
    }

    pub(crate) async fn find_character(
        &self,
        name: &str,
        world: &str,
    ) -> Result<CharacterSearchResult, LodestoneError> {
        let mut url = self
            .base_url
            .join("lodestone/character/")
            .map_err(|error| LodestoneError::Parse(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("q", name)
            .append_pair("worldname", world);

        let html = self.fetch_text(url).await?;
        let matches: Vec<_> = parse_character_search(&html)?
            .into_iter()
            .filter(|candidate| {
                candidate.name.eq_ignore_ascii_case(name)
                    && candidate.world.eq_ignore_ascii_case(world)
            })
            .collect();

        match matches.as_slice() {
            [] => Err(LodestoneError::CharacterNotFound),
            [character] => Ok(character.clone()),
            _ => Err(LodestoneError::AmbiguousCharacter),
        }
    }

    pub(crate) async fn character_profile(
        &self,
        character_id: u64,
    ) -> Result<CharacterProfile, LodestoneError> {
        let url = self
            .base_url
            .join(&format!("lodestone/character/{character_id}/"))
            .map_err(|error| LodestoneError::Parse(error.to_string()))?;
        let html = self.fetch_text(url).await?;
        parse_character_profile(&html)
    }

    pub(crate) async fn find_fc_member(
        &self,
        fc_id: &str,
        character_id: u64,
    ) -> Result<Option<FreeCompanyMember>, LodestoneError> {
        let mut page = 1_u32;
        loop {
            let html = self.fetch_fc_member_page(fc_id, page).await?;
            let (members, has_next) = parse_fc_member_page(&html, page)?;
            if page == 1 && members.is_empty() {
                return Err(LodestoneError::Parse(
                    "Free Company member page contained no roster entries".into(),
                ));
            }

            if let Some(member) = members
                .into_iter()
                .find(|member| member.character_id == character_id)
            {
                return Ok(Some(member));
            }

            if page >= 100 || !has_next {
                return Ok(None);
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            page += 1;
        }
    }

    pub(crate) async fn fc_roster(
        &self,
        fc_id: &str,
    ) -> Result<Vec<FreeCompanyMember>, LodestoneError> {
        let mut page = 1_u32;
        let mut roster = Vec::new();
        let mut seen = HashSet::new();

        loop {
            let html = self.fetch_fc_member_page(fc_id, page).await?;
            let (members, has_next) = parse_fc_member_page(&html, page)?;
            if page == 1 && members.is_empty() {
                return Err(LodestoneError::Parse(
                    "Free Company member page contained no roster entries".into(),
                ));
            }

            for member in members {
                if seen.insert(member.character_id) {
                    roster.push(member);
                }
            }

            if page >= 100 || !has_next {
                break;
            }
            tokio::time::sleep(Duration::from_millis(250)).await;
            page += 1;
        }

        Ok(roster)
    }

    async fn fetch_fc_member_page(
        &self,
        fc_id: &str,
        page: u32,
    ) -> Result<String, LodestoneError> {
        let mut url = self
            .base_url
            .join(&format!("lodestone/freecompany/{fc_id}/member/"))
            .map_err(|error| LodestoneError::Parse(error.to_string()))?;
        url.query_pairs_mut()
            .append_pair("page", &page.to_string());
        self.fetch_text(url).await
    }

    async fn fetch_text(&self, url: Url) -> Result<String, LodestoneError> {
        Ok(self
            .http
            .get(url)
            .send()
            .await?
            .error_for_status()?
            .text()
            .await?)
    }
}

fn parse_character_search(html: &str) -> Result<Vec<CharacterSearchResult>, LodestoneError> {
    let document = Html::parse_document(html);
    let entry_selector = selector("div.entry")?;
    let link_selector = selector(".entry__link")?;
    let name_selector = selector(".entry__name")?;
    let world_selector = selector(".entry__world")?;

    let mut results = Vec::new();
    for entry in document.select(&entry_selector) {
        let Some(href) = entry
            .select(&link_selector)
            .next()
            .and_then(|element| element.value().attr("href"))
        else {
            continue;
        };
        let Some(id) = extract_numeric_path_id(href, "/lodestone/character/") else {
            continue;
        };
        let Some(name) = text_of(entry.select(&name_selector).next()) else {
            continue;
        };
        let Some(world_text) = text_of(entry.select(&world_selector).next()) else {
            continue;
        };
        let world = world_text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();
        if world.is_empty() {
            continue;
        }
        results.push(CharacterSearchResult { id, name, world });
    }
    Ok(results)
}

fn parse_character_profile(html: &str) -> Result<CharacterProfile, LodestoneError> {
    let document = Html::parse_document(html);
    let bio_selector = selector(".character__selfintroduction")?;
    let fc_selector = selector(".character__freecompany__name h4 a")?;

    let biography = text_of(document.select(&bio_selector).next()).unwrap_or_default();
    let free_company_id = document
        .select(&fc_selector)
        .next()
        .and_then(|element| element.value().attr("href"))
        .and_then(|href| extract_string_path_id(href, "/lodestone/freecompany/"));

    Ok(CharacterProfile {
        biography,
        free_company_id,
    })
}

fn parse_fc_member_page(
    html: &str,
    current_page: u32,
) -> Result<(Vec<FreeCompanyMember>, bool), LodestoneError> {
    let document = Html::parse_document(html);
    let members = parse_fc_members_document(&document)?;
    let has_next = has_next_page(&document, current_page)?;
    Ok((members, has_next))
}

fn parse_fc_members_document(document: &Html) -> Result<Vec<FreeCompanyMember>, LodestoneError> {
    let entry_selector = selector("li.entry")?;
    let link_selector = selector(".entry__bg")?;
    let name_selector = selector(".entry__name")?;
    let world_selector = selector(".entry__world")?;
    let rank_selector = selector(".entry__freecompany__info > li:nth-child(1) > span")?;
    let rank_tooltip_selector = selector(".entry__freecompany__info .js__tooltip")?;

    let mut members = Vec::new();
    for entry in document.select(&entry_selector) {
        let Some(href) = entry
            .select(&link_selector)
            .next()
            .and_then(|element| element.value().attr("href"))
        else {
            continue;
        };
        let Some(character_id) = extract_numeric_path_id(href, "/lodestone/character/") else {
            continue;
        };
        let Some(name) = text_of(entry.select(&name_selector).next()) else {
            continue;
        };
        let Some(world_text) = text_of(entry.select(&world_selector).next()) else {
            continue;
        };
        let world = world_text
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_string();

        let rank = text_of(entry.select(&rank_selector).next())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                entry
                    .select(&rank_tooltip_selector)
                    .next()
                    .and_then(|element| element.value().attr("data-tooltip"))
                    .map(clean_rank_tooltip)
            })
            .unwrap_or_else(|| "Unknown".into());

        members.push(FreeCompanyMember {
            character_id,
            name,
            world,
            rank,
        });
    }
    Ok(members)
}

fn has_next_page(document: &Html, current_page: u32) -> Result<bool, LodestoneError> {
    let pager_links = selector("ul.btn__pager a")?;
    Ok(document.select(&pager_links).any(|link| {
        link.value()
            .attr("href")
            .and_then(page_from_href)
            .is_some_and(|page| page == current_page + 1)
    }))
}

fn page_from_href(href: &str) -> Option<u32> {
    let query = href.split_once('?')?.1;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        if key == "page" {
            value.parse().ok()
        } else {
            None
        }
    })
}

fn clean_rank_tooltip(value: &str) -> String {
    value
        .rsplit_once('/')
        .map(|(_, rank)| rank)
        .unwrap_or(value)
        .trim()
        .to_string()
}

fn selector(value: &str) -> Result<Selector, LodestoneError> {
    Selector::parse(value).map_err(|error| LodestoneError::Parse(error.to_string()))
}

fn text_of(element: Option<ElementRef<'_>>) -> Option<String> {
    element.map(|element| {
        element
            .text()
            .flat_map(str::split_whitespace)
            .collect::<Vec<_>>()
            .join(" ")
    })
}

fn extract_numeric_path_id(href: &str, prefix: &str) -> Option<u64> {
    extract_string_path_id(href, prefix)?.parse().ok()
}

fn extract_string_path_id(href: &str, prefix: &str) -> Option<String> {
    let suffix = href.split_once(prefix)?.1;
    let id = suffix.split('/').next()?.trim();
    (!id.is_empty()).then(|| id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_exact_character_search_fields() {
        let html = r#"
        <div class="entry">
          <a class="entry__link" href="/lodestone/character/12345/"></a>
          <p class="entry__name">Alice Blackrose</p>
          <p class="entry__world">Cactuar [Aether]</p>
        </div>
        "#;
        let results = parse_character_search(html).unwrap();
        assert_eq!(
            results,
            vec![CharacterSearchResult {
                id: 12345,
                name: "Alice Blackrose".into(),
                world: "Cactuar".into(),
            }]
        );
    }

    #[test]
    fn parses_bio_and_free_company_id() {
        let html = r#"
        <div class="character__selfintroduction">NYVEXA-ABC123 hello</div>
        <div class="character__freecompany__name"><h4><a href="/lodestone/freecompany/9234567890123456789/">Nyvexa</a></h4></div>
        "#;
        let profile = parse_character_profile(html).unwrap();
        assert_eq!(profile.biography, "NYVEXA-ABC123 hello");
        assert_eq!(
            profile.free_company_id.as_deref(),
            Some("9234567890123456789")
        );
    }

    #[test]
    fn parses_fc_member_and_rank() {
        let html = r#"
        <li class="entry">
          <a class="entry__bg" href="/lodestone/character/12345/"></a>
          <p class="entry__name">Alice Blackrose</p>
          <p class="entry__world">Cactuar [Aether]</p>
          <ul class="entry__freecompany__info"><li><span>Officer</span></li></ul>
        </li>
        "#;
        let document = Html::parse_document(html);
        let members = parse_fc_members_document(&document).unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0].character_id, 12345);
        assert_eq!(members[0].rank, "Officer");
    }

    #[test]
    fn detects_next_page_from_pager_href() {
        let html = r#"<ul class="btn__pager"><li><a href="?page=2">2</a></li></ul>"#;
        let document = Html::parse_document(html);
        assert!(has_next_page(&document, 1).unwrap());
        assert!(!has_next_page(&document, 2).unwrap());
    }
}
