use std::{collections::HashSet, time::Duration};

use reqwest::{
    Client,
    header::{ACCEPT_LANGUAGE, HeaderMap, HeaderValue},
};
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
pub(crate) struct CharacterDetails {
    pub(crate) id: u64,
    pub(crate) name: String,
    pub(crate) title: Option<String>,
    pub(crate) world: String,
    pub(crate) data_center: Option<String>,
    pub(crate) portrait_url: Option<String>,
    pub(crate) biography: String,
    pub(crate) race: Option<String>,
    pub(crate) clan: Option<String>,
    pub(crate) gender: Option<String>,
    pub(crate) nameday: Option<String>,
    pub(crate) guardian: Option<String>,
    pub(crate) city_state: Option<String>,
    pub(crate) grand_company: Option<String>,
    pub(crate) free_company_id: Option<String>,
    pub(crate) free_company_name: Option<String>,
    pub(crate) active_job: Option<ClassJob>,
    pub(crate) jobs: Vec<ClassJob>,
    pub(crate) achievement_points: Option<u32>,
    pub(crate) lodestone_url: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ClassJob {
    pub(crate) name: String,
    pub(crate) level: Option<u16>,
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
        let mut headers = HeaderMap::new();
        headers.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
        let http = Client::builder()
            .timeout(Duration::from_secs(15))
            .default_headers(headers)
            .user_agent(
                "Mozilla/5.0 (compatible; Nyvexa/0.1; +https://github.com/aliceblackrose/nyvexa)",
            )
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
        let url = self.character_page_url(character_id, "")?;
        let html = self.fetch_text(url).await?;
        parse_character_profile(&html)
    }

    pub(crate) async fn crawl_character(
        &self,
        character_id: u64,
    ) -> Result<CharacterDetails, LodestoneError> {
        let profile_url = self.character_page_url(character_id, "")?;
        let class_job_url = self.character_page_url(character_id, "class_job/")?;
        let achievement_url = self.character_page_url(character_id, "achievement/")?;

        let (profile_html, class_job_html, achievement_html) = tokio::join!(
            self.fetch_text(profile_url.clone()),
            self.fetch_text(class_job_url),
            self.fetch_text(achievement_url),
        );

        let profile_html = profile_html?;
        let mut details =
            parse_character_details(&profile_html, character_id, profile_url.as_str())?;

        if let Ok(html) = class_job_html {
            details.jobs = parse_class_jobs(&html)?;
        }
        if let Ok(html) = achievement_html {
            details.achievement_points = parse_achievement_points(&html)?;
        }

        Ok(details)
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

    fn character_page_url(&self, character_id: u64, suffix: &str) -> Result<Url, LodestoneError> {
        self.base_url
            .join(&format!("lodestone/character/{character_id}/{suffix}"))
            .map_err(|error| LodestoneError::Parse(error.to_string()))
    }

    async fn fetch_fc_member_page(&self, fc_id: &str, page: u32) -> Result<String, LodestoneError> {
        let mut url = self
            .base_url
            .join(&format!("lodestone/freecompany/{fc_id}/member/"))
            .map_err(|error| LodestoneError::Parse(error.to_string()))?;
        url.query_pairs_mut().append_pair("page", &page.to_string());
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
        let world = parse_world_and_data_center(&world_text).0;
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
    let fc_selector =
        selector(".character__freecompany__name h4 a, .character__freecompany__name a")?;

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

fn parse_character_details(
    html: &str,
    character_id: u64,
    lodestone_url: &str,
) -> Result<CharacterDetails, LodestoneError> {
    let document = Html::parse_document(html);
    let name = text_of(document.select(&selector(".frame__chara__name")?).next())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| LodestoneError::Parse("character profile is missing its name".into()))?;
    let world_text = text_of(document.select(&selector(".frame__chara__world")?).next())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| LodestoneError::Parse("character profile is missing its World".into()))?;
    let (world, data_center) = parse_world_and_data_center(&world_text);

    let title = optional_text(&document, ".frame__chara__title")?;
    let biography = optional_text(&document, ".character__selfintroduction")?.unwrap_or_default();
    let portrait_url = first_attribute(
        &document,
        ".character__detail__image img, .character__view__img, .frame__chara__face img",
        "src",
    )?
    .map(upgrade_portrait_url);
    let free_company_link = document
        .select(&selector(
            ".character__freecompany__name h4 a, .character__freecompany__name a",
        )?)
        .next();
    let free_company_id = free_company_link
        .as_ref()
        .and_then(|element| element.value().attr("href"))
        .and_then(|href| extract_string_path_id(href, "/lodestone/freecompany/"));
    let free_company_name = text_of(free_company_link);

    let mut race = None;
    let mut clan = None;
    let mut gender = None;
    let mut nameday = None;
    let mut guardian = None;
    let mut city_state = None;
    let mut grand_company = None;

    let block_selector = selector(".character__profile__data__detail .character-block")?;
    let block_title_selector = selector(".character-block__title")?;
    let block_name_selector = selector(".character-block__name")?;
    let block_birth_selector = selector(".character-block__birth")?;

    for block in document.select(&block_selector) {
        let Some(block_title) = text_of(block.select(&block_title_selector).next()) else {
            continue;
        };
        let block_name = block.select(&block_name_selector).next();
        let parts = block_name.map(text_parts).unwrap_or_default();

        if matches_label(
            &block_title,
            &[
                "Race/Clan/Gender",
                "Volk / Stamm / Geschlecht",
                "Race / Ethnie / Sexe",
                "種族/部族/性別",
            ],
        ) {
            race = parts.first().cloned();
            clan = parts.get(1).cloned();
            gender = parts.get(2).map(|value| normalize_gender(value));
        } else if matches_label(
            &block_title,
            &["Nameday", "Namenstag", "Date de naissance", "誕生日"],
        ) {
            nameday = text_of(block.select(&block_birth_selector).next())
                .or_else(|| (!parts.is_empty()).then(|| parts.join(" / ")));
        } else if matches_label(
            &block_title,
            &["Guardian", "Schutzgott", "Divinité", "守護神"],
        ) {
            guardian = (!parts.is_empty()).then(|| parts.join(" / "));
        } else if matches_label(
            &block_title,
            &["City-state", "Stadtstaat", "Cité de départ", "開始都市"],
        ) {
            city_state = (!parts.is_empty()).then(|| parts.join(" / "));
        } else if matches_label(
            &block_title,
            &[
                "Grand Company",
                "Staatliche Gesellschaft",
                "Grande compagnie",
                "所属グランドカンパニー",
            ],
        ) {
            grand_company = (!parts.is_empty()).then(|| parts.join(" / "));
        }
    }

    let active_job = parse_active_job(&document)?;

    Ok(CharacterDetails {
        id: character_id,
        name,
        title,
        world,
        data_center,
        portrait_url,
        biography,
        race,
        clan,
        gender,
        nameday,
        guardian,
        city_state,
        grand_company,
        free_company_id,
        free_company_name,
        active_job,
        jobs: Vec::new(),
        achievement_points: None,
        lodestone_url: lodestone_url.to_string(),
    })
}

fn parse_active_job(document: &Html) -> Result<Option<ClassJob>, LodestoneError> {
    let class_data_selector = selector(".character__class__data")?;
    let Some(class_data) = document.select(&class_data_selector).next() else {
        return Ok(None);
    };

    let level = parse_level(&text_parts(class_data).join(" "));
    let icon_selector = selector(".character__class_icon img, img")?;
    let name = class_data.select(&icon_selector).find_map(|image| {
        image
            .value()
            .attr("data-tooltip")
            .or_else(|| image.value().attr("alt"))
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    });

    Ok(name.map(|name| ClassJob { name, level }))
}

fn parse_class_jobs(html: &str) -> Result<Vec<ClassJob>, LodestoneError> {
    let document = Html::parse_document(html);
    let entry_selector = selector(".character__job li, .character__job__role li")?;
    let name_selector = selector(".character__job__name")?;
    let level_selector = selector(".character__job__level")?;
    let image_selector = selector("img")?;
    let mut seen = HashSet::new();
    let mut jobs = Vec::new();

    for entry in document.select(&entry_selector) {
        let name = text_of(entry.select(&name_selector).next())
            .filter(|value| !value.is_empty())
            .or_else(|| {
                entry.select(&image_selector).find_map(|image| {
                    image
                        .value()
                        .attr("data-tooltip")
                        .or_else(|| image.value().attr("alt"))
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .map(str::to_string)
                })
            });
        let Some(name) = name else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }

        let level = text_of(entry.select(&level_selector).next())
            .as_deref()
            .and_then(parse_level);
        jobs.push(ClassJob { name, level });
    }

    Ok(jobs)
}

fn parse_achievement_points(html: &str) -> Result<Option<u32>, LodestoneError> {
    let document = Html::parse_document(html);
    let point_selector = selector(".achievement__point")?;
    Ok(document
        .select(&point_selector)
        .find_map(|element| parse_u32(&text_parts(element).join(" "))))
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
        let world = parse_world_and_data_center(&world_text).0;

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

fn optional_text(document: &Html, selector_value: &str) -> Result<Option<String>, LodestoneError> {
    Ok(text_of(document.select(&selector(selector_value)?).next())
        .filter(|value| !value.is_empty()))
}

fn first_attribute(
    document: &Html,
    selector_value: &str,
    attribute: &str,
) -> Result<Option<String>, LodestoneError> {
    Ok(document
        .select(&selector(selector_value)?)
        .find_map(|element| element.value().attr(attribute))
        .map(str::to_string))
}

fn text_of(element: Option<ElementRef<'_>>) -> Option<String> {
    element.map(|element| text_parts(element).join(" "))
}

fn text_parts(element: ElementRef<'_>) -> Vec<String> {
    element
        .text()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_world_and_data_center(value: &str) -> (String, Option<String>) {
    let value = value.trim();
    if let Some((world, data_center)) = value.rsplit_once('[') {
        let data_center = data_center.trim().trim_end_matches(']').trim();
        if !world.trim().is_empty() && !data_center.is_empty() {
            return (world.trim().to_string(), Some(data_center.to_string()));
        }
    }
    (value.to_string(), None)
}

fn normalize_gender(value: &str) -> String {
    match value.trim() {
        "♀" => "Female".into(),
        "♂" => "Male".into(),
        value => value.to_string(),
    }
}

fn matches_label(value: &str, labels: &[&str]) -> bool {
    labels.iter().any(|label| value.eq_ignore_ascii_case(label))
}

fn upgrade_portrait_url(value: String) -> String {
    value.replace("c0_96x96", "l0_640x873")
}

fn parse_level(value: &str) -> Option<u16> {
    value
        .split(|character: char| !character.is_ascii_digit())
        .find_map(|part| (!part.is_empty()).then(|| part.parse().ok()).flatten())
}

fn parse_u32(value: &str) -> Option<u32> {
    let digits: String = value.chars().filter(char::is_ascii_digit).collect();
    (!digits.is_empty()).then(|| digits.parse().ok()).flatten()
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
    fn parses_rich_character_profile() {
        let html = r#"
        <div class="frame__chara">
          <p class="frame__chara__title">The Azure Dragoon</p>
          <p class="frame__chara__name">Cybelle Reaper</p>
          <p class="frame__chara__world">Alpha [Light]</p>
          <div class="frame__chara__face"><img src="https://img2.finalfantasyxiv.com/f/abc_c0_96x96.jpg"></div>
        </div>
        <div class="character__profile__data__detail">
          <div class="character-block">
            <p class="character-block__title">Race/Clan/Gender</p>
            <p class="character-block__name">Au Ra<br>Raen<br>♀</p>
          </div>
          <div class="character-block">
            <p class="character-block__title">Nameday</p>
            <p class="character-block__birth">1st Sun of the 1st Astral Moon</p>
          </div>
          <div class="character-block">
            <p class="character-block__title">Guardian</p>
            <p class="character-block__name">Azeyma, the Warden</p>
          </div>
          <div class="character-block">
            <p class="character-block__title">City-state</p>
            <p class="character-block__name">Gridania</p>
          </div>
          <div class="character-block">
            <p class="character-block__title">Grand Company</p>
            <p class="character-block__name">Order of the Twin Adder<br>Serpent Captain</p>
          </div>
        </div>
        <div class="character__class__data"><img data-tooltip="Dragoon"><p>LEVEL 100</p></div>
        <div class="character__freecompany__name"><h4><a href="/lodestone/freecompany/9234567890123456789/">Nyvexa</a></h4></div>
        <div class="character__selfintroduction">Hello from Eorzea.</div>
        "#;
        let details = parse_character_details(
            html,
            58_164_347,
            "https://eu.finalfantasyxiv.com/lodestone/character/58164347/",
        )
        .unwrap();

        assert_eq!(details.name, "Cybelle Reaper");
        assert_eq!(details.world, "Alpha");
        assert_eq!(details.data_center.as_deref(), Some("Light"));
        assert_eq!(details.race.as_deref(), Some("Au Ra"));
        assert_eq!(details.clan.as_deref(), Some("Raen"));
        assert_eq!(details.gender.as_deref(), Some("Female"));
        assert_eq!(details.guardian.as_deref(), Some("Azeyma, the Warden"));
        assert_eq!(details.city_state.as_deref(), Some("Gridania"));
        assert_eq!(details.active_job.as_ref().unwrap().name, "Dragoon");
        assert_eq!(details.active_job.as_ref().unwrap().level, Some(100));
        assert!(details.portrait_url.unwrap().contains("l0_640x873"));
    }

    #[test]
    fn parses_class_jobs() {
        let html = r#"
        <ul class="character__job">
          <li><p class="character__job__name">Paladin</p><p class="character__job__level">100</p></li>
          <li><p class="character__job__name">Dragoon</p><p class="character__job__level">90</p></li>
          <li><img data-tooltip="Viper"><p class="character__job__level">-</p></li>
        </ul>
        "#;
        assert_eq!(
            parse_class_jobs(html).unwrap(),
            vec![
                ClassJob {
                    name: "Paladin".into(),
                    level: Some(100),
                },
                ClassJob {
                    name: "Dragoon".into(),
                    level: Some(90),
                },
                ClassJob {
                    name: "Viper".into(),
                    level: None,
                },
            ]
        );
    }

    #[test]
    fn parses_achievement_points() {
        let html = r#"<div class="achievement__point">12,345</div>"#;
        assert_eq!(parse_achievement_points(html).unwrap(), Some(12_345));
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
