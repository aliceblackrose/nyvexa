use anyhow::Result as AppResult;
use gloam_commands::prelude::*;
use gloamwire::model::{Embed, EmbedField, EmbedFooter, EmbedMedia};
use tracing::warn;

use crate::{
    lodestone::{CharacterDetails, ClassJob},
    store::VerifiedMember,
};

use super::{CommandData, finish_embed, invoking_user_id};

const STATUS_COLOR: u32 = 0xD6A64B;
const WARNING_COLOR: u32 = 0xE6A23C;
const INACTIVE_COLOR: u32 = 0x747F8D;

#[command(
    description = "Show your live FFXIV character and Nyvexa verification status",
    guild_only
)]
pub(crate) async fn status(ctx: Context<CommandData>) -> Result<()> {
    ctx.defer_ephemeral().await?;
    let result = run(&ctx).await;
    finish_embed(&ctx, result).await
}

async fn run(ctx: &Context<CommandData>) -> AppResult<Embed> {
    let user_id = invoking_user_id(ctx)?;
    let Some(member) = ctx.data().store.verified(user_id.get()).await? else {
        return Ok(unverified_embed());
    };

    match ctx
        .data()
        .lodestone
        .crawl_character(member.character_id)
        .await
    {
        Ok(details) => Ok(character_embed(&member, &details)),
        Err(error) => {
            warn!(
                ?error,
                character_id = member.character_id,
                "live Lodestone status crawl failed"
            );
            Ok(stored_status_embed(&member))
        }
    }
}

fn character_embed(member: &VerifiedMember, details: &CharacterDetails) -> Embed {
    let mut fields = Vec::new();

    push_field(
        &mut fields,
        "Verification",
        if member.missing_since.is_some() {
            "⚠️ Verified link\nFC membership pending revalidation".into()
        } else {
            "✅ Verified\nFC membership active".into()
        },
        true,
    );
    push_field(
        &mut fields,
        "World",
        details
            .data_center
            .as_ref()
            .map(|data_center| format!("{} [{}]", details.world, data_center))
            .unwrap_or_else(|| details.world.clone()),
        true,
    );
    push_field(
        &mut fields,
        "Character ID",
        details.id.to_string(),
        true,
    );

    let rank = member.fc_rank.as_deref().unwrap_or("Unknown");
    let free_company_name = details.free_company_name.as_deref().unwrap_or("Free Company");
    let free_company = details
        .free_company_id
        .as_deref()
        .map(|id| {
            let base = details
                .lodestone_url
                .split("/lodestone/character/")
                .next()
                .unwrap_or_default();
            format!(
                "[{}]({}/lodestone/freecompany/{}/)\nRank: **{}**",
                free_company_name, base, id, rank
            )
        })
        .unwrap_or_else(|| format!("{}\nRank: **{}**", free_company_name, rank));
    push_field(&mut fields, "Free Company", free_company, true);

    if let Some(active_job) = details.active_job.as_ref() {
        push_field(
            &mut fields,
            "Current Job",
            format_job(active_job),
            true,
        );
    }
    if let Some(points) = details.achievement_points {
        push_field(
            &mut fields,
            "Achievement Points",
            format_number(points),
            true,
        );
    }

    let profile = [
        details.race.as_deref(),
        details.clan.as_deref(),
        details.gender.as_deref(),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" • ");
    if !profile.is_empty() {
        push_field(&mut fields, "Profile", profile, true);
    }
    if let Some(city_state) = details.city_state.as_ref() {
        push_field(&mut fields, "City-state", city_state.clone(), true);
    }
    if let Some(grand_company) = details.grand_company.as_ref() {
        push_field(
            &mut fields,
            "Grand Company",
            grand_company.clone(),
            true,
        );
    }
    if let Some(guardian) = details.guardian.as_ref() {
        push_field(&mut fields, "Guardian", guardian.clone(), true);
    }
    if let Some(nameday) = details.nameday.as_ref() {
        push_field(&mut fields, "Nameday", nameday.clone(), true);
    }

    if let Some(jobs) = format_jobs(&details.jobs, false) {
        push_field(&mut fields, "Battle Jobs", jobs, false);
    }
    if let Some(jobs) = format_jobs(&details.jobs, true) {
        push_field(&mut fields, "Crafting & Gathering", jobs, false);
    }

    let description = profile_description(details);

    Embed {
        title: Some(details.name.clone()),
        description: (!description.is_empty()).then_some(description),
        url: Some(details.lodestone_url.clone()),
        color: Some(STATUS_COLOR),
        footer: Some(EmbedFooter {
            text: "Nyvexa • Live Lodestone crawl • Public profile data".into(),
            icon_url: None,
            proxy_icon_url: None,
        }),
        image: details.portrait_url.as_ref().map(|url| EmbedMedia {
            url: Some(url.clone()),
            ..Default::default()
        }),
        fields,
        ..Default::default()
    }
}

fn stored_status_embed(member: &VerifiedMember) -> Embed {
    let membership = if member.missing_since.is_some() {
        "⚠️ FC membership is pending revalidation."
    } else {
        "✅ FC membership is active."
    };

    Embed {
        title: Some(member.character_name.clone()),
        description: Some(format!(
            "{}\n\nLive Lodestone profile data is temporarily unavailable, so this card is using Nyvexa's stored verification data.",
            membership
        )),
        color: Some(WARNING_COLOR),
        fields: vec![
            EmbedField {
                name: "World".into(),
                value: member.world.clone(),
                inline: Some(true),
            },
            EmbedField {
                name: "FC Rank".into(),
                value: member.fc_rank.as_deref().unwrap_or("Unknown").into(),
                inline: Some(true),
            },
            EmbedField {
                name: "Character ID".into(),
                value: member.character_id.to_string(),
                inline: Some(true),
            },
        ],
        footer: Some(EmbedFooter {
            text: "Nyvexa • Stored verification data".into(),
            icon_url: None,
            proxy_icon_url: None,
        }),
        ..Default::default()
    }
}

fn unverified_embed() -> Embed {
    Embed {
        title: Some("Nyvexa Verification".into()),
        description: Some(
            "You are not verified yet. Run `/verify` with your exact FFXIV character name and World to begin."
                .into(),
        ),
        color: Some(INACTIVE_COLOR),
        footer: Some(EmbedFooter {
            text: "Nyvexa • FFXIV Free Company verification".into(),
            icon_url: None,
            proxy_icon_url: None,
        }),
        ..Default::default()
    }
}

fn profile_description(details: &CharacterDetails) -> String {
    let mut parts = Vec::new();
    if let Some(title) = details.title.as_deref() {
        parts.push(format!("*{}*", truncate(title, 180)));
    }
    if !details.biography.trim().is_empty() {
        parts.push(truncate(&sanitize_biography(&details.biography), 650));
    }
    parts.join("\n\n")
}

fn sanitize_biography(value: &str) -> String {
    value
        .split_whitespace()
        .map(|part| {
            if part.starts_with("NYVEXA-") {
                "[verification code hidden]"
            } else {
                part
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_jobs(jobs: &[ClassJob], crafting: bool) -> Option<String> {
    let mut output = String::new();

    for job in jobs {
        if is_crafting_or_gathering(&job.name) != crafting {
            continue;
        }
        let Some(level) = job.level else {
            continue;
        };
        let part = format!("**{}** {}", job_abbreviation(&job.name), level);
        let separator = if output.is_empty() { "" } else { " • " };
        if output.chars().count() + separator.chars().count() + part.chars().count() > 1000 {
            output.push_str(" …");
            break;
        }
        output.push_str(separator);
        output.push_str(&part);
    }

    (!output.is_empty()).then_some(output)
}

fn format_job(job: &ClassJob) -> String {
    match job.level {
        Some(level) => format!("**{}** • Level {}", job.name, level),
        None => format!("**{}**", job.name),
    }
}

fn is_crafting_or_gathering(name: &str) -> bool {
    matches!(
        name,
        "Carpenter"
            | "Blacksmith"
            | "Armorer"
            | "Goldsmith"
            | "Leatherworker"
            | "Weaver"
            | "Alchemist"
            | "Culinarian"
            | "Miner"
            | "Botanist"
            | "Fisher"
    )
}

fn job_abbreviation(name: &str) -> &str {
    match name {
        "Paladin" => "PLD",
        "Warrior" => "WAR",
        "Dark Knight" => "DRK",
        "Gunbreaker" => "GNB",
        "White Mage" => "WHM",
        "Scholar" => "SCH",
        "Astrologian" => "AST",
        "Sage" => "SGE",
        "Monk" => "MNK",
        "Dragoon" => "DRG",
        "Ninja" => "NIN",
        "Samurai" => "SAM",
        "Reaper" => "RPR",
        "Viper" => "VPR",
        "Bard" => "BRD",
        "Machinist" => "MCH",
        "Dancer" => "DNC",
        "Black Mage" => "BLM",
        "Summoner" => "SMN",
        "Red Mage" => "RDM",
        "Pictomancer" => "PCT",
        "Blue Mage" => "BLU",
        "Carpenter" => "CRP",
        "Blacksmith" => "BSM",
        "Armorer" => "ARM",
        "Goldsmith" => "GSM",
        "Leatherworker" => "LTW",
        "Weaver" => "WVR",
        "Alchemist" => "ALC",
        "Culinarian" => "CUL",
        "Miner" => "MIN",
        "Botanist" => "BTN",
        "Fisher" => "FSH",
        name => name,
    }
}

fn push_field(fields: &mut Vec<EmbedField>, name: &str, value: String, inline: bool) {
    if value.trim().is_empty() {
        return;
    }
    fields.push(EmbedField {
        name: name.into(),
        value: truncate(&value, 1024),
        inline: Some(inline),
    });
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}…", truncated.trim_end())
    } else {
        truncated
    }
}

fn format_number(value: u32) -> String {
    let digits = value.to_string();
    let mut output = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, character) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            output.push(',');
        }
        output.push(character);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hides_verification_challenges_from_profile_bio() {
        assert_eq!(
            sanitize_biography("Hello NYVEXA-0123456789ABCDEF goodbye"),
            "Hello [verification code hidden] goodbye"
        );
    }

    #[test]
    fn formats_job_groups_compactly() {
        let jobs = vec![
            ClassJob {
                name: "Paladin".into(),
                level: Some(100),
            },
            ClassJob {
                name: "Dragoon".into(),
                level: Some(90),
            },
            ClassJob {
                name: "Carpenter".into(),
                level: Some(80),
            },
        ];
        assert_eq!(
            format_jobs(&jobs, false).as_deref(),
            Some("**PLD** 100 • **DRG** 90")
        );
        assert_eq!(format_jobs(&jobs, true).as_deref(), Some("**CRP** 80"));
    }

    #[test]
    fn adds_grouping_to_large_numbers() {
        assert_eq!(format_number(12_345_678), "12,345,678");
    }
}
