use axum::{extract::State, response::Html};
use minijinja::context;

use crate::app::error::WebError;
use crate::app::picture;
use crate::app::state::AppState;

struct FaqCategory {
    title: &'static str,
    items: &'static [FaqItem],
}

struct FaqItem {
    question: &'static str,
    answer: &'static str,
}

const FAQS: &[FaqCategory] = &[
    FaqCategory {
        title: "Getting started",
        items: &[
            FaqItem {
                question: "How do I get CheckStitch?",
                answer: "CheckStitch is being prepared for its App Store release on iPhone, iPad, and Mac, with a matching Apple Watch app included. There is no account and no setup: install it, open it, and tap + to name your first checklist.",
            },
            FaqItem {
                question: "How do I run my first checklist?",
                answer: "Tap +, give the checklist a name, and add your items — each one gets a title and an optional note. Pick the Reminders list the items should go to, then tap Create reminders. One reminder is created for each item, and CheckStitch leaves them alone after that.",
            },
        ],
    },
    FaqCategory {
        title: "Features",
        items: &[
            FaqItem {
                question: "What exactly does CheckStitch create?",
                answer: "One Apple Reminder per item. The item's name becomes the reminder's title and the item's note becomes the reminder's note. Those reminders are then yours to use like any other — CheckStitch never reads, edits, completes, or deletes them afterwards.",
            },
            FaqItem {
                question: "Can I choose where the reminders go?",
                answer: "Yes. Every checklist has a destination list — your Reminders inbox or any list you already have. You can change it as often as you like while editing the checklist.",
            },
            FaqItem {
                question: "What is Number Reminders?",
                answer: "A toggle that prefixes each reminder's title with its position, like \"1. Buy milk\", so the order you wrote survives the trip into Reminders. Leave it off when the order doesn't matter.",
            },
            FaqItem {
                question: "Can I reuse a checklist?",
                answer: "That is the point. A checklist is a reusable template: go back to it next week, next month, or next Monday morning, and run it again to recreate the reminders without retyping a thing.",
            },
            FaqItem {
                question: "Does CheckStitch work on the Apple Watch?",
                answer: "Yes. Installing CheckStitch on your iPhone also installs a watch app, so you can read your checklists and create their reminders from your wrist without pulling out your phone.",
            },
            FaqItem {
                question: "Do my checklists sync between my devices?",
                answer: "Yes. Your checklists are stored on your device and synced through your own iCloud account, so the checklist you built on your iPhone is waiting for you on your iPad and Mac.",
            },
        ],
    },
    FaqCategory {
        title: "Privacy",
        items: &[
            FaqItem {
                question: "Do you collect my data?",
                answer: "No. CheckStitch has no analytics, no tracking, and no advertising. Your reminders and checklists are never sent to the author or to any third party.",
            },
            FaqItem {
                question: "Does CheckStitch read or change my existing reminders?",
                answer: "No. It only creates reminders from your checklist items. It never reads, edits, completes, or deletes reminders after they have been created.",
            },
            FaqItem {
                question: "Why does the app use the network at all?",
                answer: "Only for the optional background. The wallpaper and artist credit are fetched through a proxy at vardy.cc, and that request never includes any reminder, checklist, or preference data. With the background switched off, CheckStitch makes no network requests at all.",
            },
        ],
    },
    FaqCategory {
        title: "Other",
        items: &[
            FaqItem {
                question: "Can I back up my checklists?",
                answer: "Yes. Settings has Export and Import, so you can save your checklists to a file, or move them onto another device whenever you like.",
            },
            FaqItem {
                question: "Are you going to create an Android version?",
                answer: "There are no current plans for one. CheckStitch is built entirely on Apple Reminders and the Apple platforms. If this is something you would like, send me an email!",
            },
            FaqItem {
                question: "How do I get in touch?",
                answer: "Use the contact form or send an email. I read everything, and it is the fastest way to get a feature or a fix into the app.",
            },
        ],
    },
];

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("checkstitch");
    // The wallpaper and its photographer credit are decorative fallbacks:
    // render the page without them rather than failing the whole request
    // if Unsplash is unavailable.
    let (wallpaper_url, photographer, photographer_url) = picture::wallpaper_context(&state).await;
    // FAQS cannot implement serde::Serialize directly (the interfaces layer
    // may not depend on `serde`), so marshal it through serde_json for
    // minijinja's context! macro, which requires Serialize values.
    let faq_categories: Vec<serde_json::Value> = FAQS
        .iter()
        .map(|category| {
            let items: Vec<serde_json::Value> = category
                .items
                .iter()
                .map(|item| serde_json::json!({ "question": item.question, "answer": item.answer }))
                .collect();
            serde_json::json!({ "title": category.title, "items": items })
        })
        .collect();
    let html = state.templates.get_template("checkstitch.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "checkstitch", faq_categories },
    )?;
    Ok(Html(html))
}

#[cfg(test)]
mod tests {
    use super::{FAQS, FaqItem};

    /// Flatten every FAQ item across categories for assertions that span the
    /// whole FAQ content.
    fn all_faq_items() -> Vec<&'static FaqItem> {
        FAQS.iter()
            .flat_map(|category| category.items.iter())
            .collect()
    }
    use crate::test::{
        seed_wallpaper_no_url, start_app, start_app_with, start_unsplash_stub, test_client,
    };
    use axum::http::StatusCode;

    #[tokio::test]
    async fn index_serves_ok_html() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        assert!(
            res.headers()
                .get("content-type")
                .is_some_and(|v| v.to_str().unwrap().contains("text/html"))
        );
        let body = res.text().await.unwrap();
        assert!(body.contains("<title>CheckStitch</title>"));
        assert!(body.contains("<h1>CheckStitch</h1>"));
        assert!(body.contains("Your list is how you think")); // hero tagline
        assert!(body.contains("One checklist in. A list of reminders out.")); // closing CTA
        assert!(body.contains(r#"<a href="/checkstitch" class="active">CheckStitch</a>"#));
        // screenshot gallery + feature section headings
        assert!(body.contains("Why it helps"));
        assert!(body.contains("Everything you need, nothing you don't"));
        assert!(body.contains("Thoughtful by design"));
        assert!(body.contains("Built for quiet productivity"));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-main.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-edit.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-settings.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-ipad.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-shot-ipad-edit.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-watch-list.png?v="#));
        assert!(body.contains(r#"<img src="/static/checkstitch-watch-create.png?v="#));
        assert!(body.contains("Apple Watch showing the Create reminders button"));
        // every asset_url reference must resolve — a missing file panics the handler
        assert_eq!(body.matches("app-store.svg").count(), 0);
        // No App Store badge: CheckStitch has no App Store presence yet.
        assert!(!body.contains("apps.apple.com"));
        assert!(!body.contains("app-store.svg"));
        // server-rendered wallpaper from the seeded cache row; minijinja
        // escapes `/` in attribute context, browsers decode it back
        assert!(body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')"));
        // credit line appears for the photographer
        assert!(body.contains("Photo by"));
        // responsive: wallpaper and credit are hidden on mobile breakpoints
        assert!(body.contains(r#"class="wallpaper hidden md:block""#));
        assert!(body.contains("hidden md:block"));
        assert!(body.contains("bg-black/50"));
    }

    #[tokio::test]
    async fn index_still_renders_when_wallpaper_fetch_fails() {
        let stub = start_unsplash_stub(axum::http::StatusCode::INTERNAL_SERVER_ERROR).await;
        let (addr, db) = start_app_with(&stub.base_url).await;
        sqlx::query("DELETE FROM unsplash_pictures")
            .execute(&db)
            .await
            .expect("clear pictures");

        let res = test_client()
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.expect("body");
        assert!(!body.contains("background-image"));
        assert!(!body.contains("Photo by"));
    }

    #[tokio::test]
    async fn index_shows_credit_as_text_when_no_photographer_url() {
        let (addr, db) = start_app_with("https://api.unsplash.com").await;
        sqlx::query("DELETE FROM unsplash_pictures")
            .execute(&db)
            .await
            .expect("clear pictures");
        seed_wallpaper_no_url(&db).await;
        let body = test_client()
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        assert!(body.contains("Photo by NoLink Photographer on Unsplash"));
        // The name must NOT be wrapped in a link when photographer_url is empty
        assert!(!body.contains("NoLink Photographer</a>"));
    }

    /// Reproduce minijinja's HTML autoescape for the characters it escapes
    /// (see `AutoEscape::Html` docs: `<`, `>`, `&`, `"`, `'`, `/`).
    fn html_escape(input: &str) -> String {
        input
            .replace('&', "&amp;")
            .replace('<', "&lt;")
            .replace('>', "&gt;")
            .replace('"', "&quot;")
            .replace('\'', "&#x27;")
            .replace('/', "&#x2f;")
    }

    #[tokio::test]
    async fn faq_all_questions_appear() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        for item in all_faq_items() {
            let escaped_question = html_escape(item.question);
            assert!(
                body.contains(&escaped_question),
                "FAQ question not found in rendered page: {}",
                item.question,
            );
        }
    }

    #[tokio::test]
    async fn faq_all_answers_appear() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        for item in all_faq_items() {
            let escaped_answer = html_escape(item.answer);
            assert!(
                body.contains(&escaped_answer),
                "FAQ answer not found in rendered page: {}",
                &item.answer[..item.answer.len().min(60)],
            );
        }
    }

    #[tokio::test]
    async fn faq_privacy_disclosures_documented() {
        let addr = start_app().await;
        let body = test_client()
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        let heading = "<h3 class=\"heading-subsection\">Privacy</h3>";
        assert!(body.contains(heading), "Privacy category heading missing");
        let question = html_escape("Do you collect my data?");
        assert!(
            body.contains(&question),
            "privacy question missing from rendered page"
        );
        let answer = html_escape("no analytics, no tracking, and no advertising");
        assert!(
            body.contains(&answer),
            "privacy answer missing from rendered page"
        );
    }

    #[tokio::test]
    async fn faq_section_after_quiet_productivity_before_cta() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();

        let quiet_pos = body
            .find("Built for quiet productivity")
            .expect("quiet productivity heading");
        let faq_pos = body
            .find("Frequently Asked Questions")
            .expect("FAQ heading");
        let cta_pos = body
            .find("One checklist in. A list of reminders out.")
            .expect("CTA");

        assert!(
            quiet_pos < faq_pos,
            "FAQ must appear after 'Built for quiet productivity'"
        );
        assert!(faq_pos < cta_pos, "FAQ must appear before closing CTA");
    }

    #[tokio::test]
    async fn faq_no_javascript() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        assert!(!body.contains("<script"));
        assert!(!body.contains("onclick"));
    }

    #[tokio::test]
    async fn faq_summary_has_chevron() {
        let addr = start_app().await;
        let client = test_client();
        let body = client
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        for item in all_faq_items() {
            let question = html_escape(item.question);
            assert!(
                body.contains(&format!("</span>{question}")),
                "chevron missing before question: {}",
                item.question,
            );
        }
    }

    #[tokio::test]
    async fn faq_items_grouped_under_category_headings() {
        let addr = start_app().await;
        let body = test_client()
            .get(format!("http://{addr}/checkstitch"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");

        for category in FAQS {
            let heading = format!("<h3 class=\"heading-subsection\">{}</h3>", category.title);
            assert!(
                body.contains(&heading),
                "FAQ category heading not found: {}",
                category.title,
            );
        }

        for (i, category) in FAQS.iter().enumerate() {
            let heading = format!("<h3 class=\"heading-subsection\">{}</h3>", category.title);
            let heading_pos = body.find(&heading).expect("category heading");
            let first_question = html_escape(category.items[0].question);
            let question_pos = body
                .find(&first_question)
                .expect("first question of category");
            assert!(
                heading_pos < question_pos,
                "question '{}' must appear after its category heading '{}'",
                category.items[0].question,
                category.title,
            );
            if let Some(next) = FAQS.get(i + 1) {
                let next_heading = format!("<h3 class=\"heading-subsection\">{}</h3>", next.title);
                let next_pos = body.find(&next_heading).expect("next category heading");
                assert!(
                    question_pos < next_pos,
                    "question '{}' must appear before the next category heading '{}'",
                    category.items[0].question,
                    next.title,
                );
            }
        }
    }

    #[test]
    fn faq_items_all_non_empty() {
        for (i, category) in FAQS.iter().enumerate() {
            assert!(
                !category.title.is_empty(),
                "FAQ category {i} has empty title"
            );
            assert!(!category.items.is_empty(), "FAQ category {i} has no items");
        }
        for (i, item) in all_faq_items().iter().enumerate() {
            assert!(!item.question.is_empty(), "FAQ item {i} has empty question");
            assert!(!item.answer.is_empty(), "FAQ item {i} has empty answer");
        }
    }

    #[test]
    fn faq_items_no_duplicate_questions() {
        let mut seen = std::collections::HashSet::new();
        for item in all_faq_items() {
            assert!(
                seen.insert(item.question),
                "Duplicate FAQ question: {}",
                item.question
            );
        }
    }
}
