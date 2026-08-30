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
                question: "How do I get the app?",
                answer: "Download SingleThread from the App Store on your iPhone and iPad. It reads your existing Apple Reminders — no import, no setup, no account. Open the app and you'll see your first reminder right away. From there, tap Complete, Skip, or Delete, and the next one appears.",
            },
            FaqItem {
                question: "Is SingleThread free?",
                answer: "SingleThread is free to use until you reach 100 reminder completions, and then there is a small $USD 2.99 one-time unlock fee to enjoy unlimited usage.",
            },
        ],
    },
    FaqCategory {
        title: "Features",
        items: &[
            FaqItem {
                question: "Do you support the Apple Watch?",
                answer: "Yes! This is a top priority for me and installing this app on your iPhone also installs an accompanying app on your Apple Watch.",
            },
            FaqItem {
                question: "Are you going to create an Android version?",
                answer: "I'm not against the idea, but there are no current plans to do so. If this is something that you would like, send me an email!",
            },
            FaqItem {
                question: "Are you planning on supporting other task managers?",
                answer: "I have been toying with the idea of supporting more task managers, please let me know if this is something that you desire and for which task manager.",
            },
            FaqItem {
                question: "Why can't I see url or file attachments?",
                answer: "I would love to implement this but Apple's internal API for this has been broken for a while, if or when this is fixed I would be delighted to add it.",
            },
            FaqItem {
                question: "Why don't you have dictation on the watch?",
                answer: "Because the functionality is already built in! I hold down the crown and say something along the lines of \"Remind me to get the milk today\" and it will create a reminder with today's date on it.",
            },
            FaqItem {
                question: "Does this app work off-line?",
                answer: "It sure does! The changes to your reminders are stored on your device and will be synced to iCloud when you're next online. During this time, you will not be able to fetch new wallpapers, but the app will degrade gracefully in this case.",
            },
        ],
    },
    FaqCategory {
        title: "Privacy",
        items: &[
            FaqItem {
                question: "Where is my data stored?",
                answer: "Your Reminders are stored on your phone and on iCloud in Apple Reminders. Your settings are stored on your device and in iCloud. I do not store any of your information myself. The only way that I will find out anything about you is if you email me.",
            },
            FaqItem {
                question: "What network requests does this app make?",
                answer: "I only have the app perform network requests to fetch new wallpapers.",
            },
            FaqItem {
                question: "Do you collect or sell my data?",
                answer: "Absolutely not. I do not retain, store, use, or sell any information about you.",
            },
        ],
    },
    FaqCategory {
        title: "Other",
        items: &[
            FaqItem {
                question: "Why did you choose Apple Reminders?",
                answer: "I chose Apple Reminders because it is free, has first class support on Apple devices, and is a pragmatic choice for many Apple users. I recommend using both Apple Reminders and SingleThread alongside each other because they both have different use-cases.",
            },
            FaqItem {
                question: "Where are the wallpapers from and how do you select them?",
                answer: "The wallpapers are from Unsplash. My server at vardy.cc fetches random nature wallpapers from their service and caches them. The app then gets the wallpapers from my server using no identifying information about you. This allows me to obscure my API key and keep the number of requests to Unsplash to a reasonable level.",
            },
            FaqItem {
                question: "Pulp or no pulp?",
                answer: "I try not to be too picky, but I definitely prefer pulp.",
            },
            FaqItem {
                question: "Can I contact you with questions, bug reports, or feature requests?",
                answer: "I would appreciate it! Please use my contact form and I will read your email personally.",
            },
        ],
    },
];

pub async fn index(State(state): State<AppState>) -> Result<Html<String>, WebError> {
    state.metrics.inc_page_view("singlethread");
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
    let html = state.templates.get_template("singlethread.html")?.render(
        context! { wallpaper_url, photographer, photographer_url, active_page => "singlethread", faq_categories },
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
            .get(format!("http://{addr}/singlethread"))
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
        assert!(body.contains("<title>SingleThread</title>"));
        assert!(body.contains("<h1>SingleThread</h1>"));
        assert!(body.contains("Your brain does one thing at a time")); // hero tagline
        assert!(body.contains("One at a time.")); // first bullet lead-in
        assert!(body.contains("Why it helps"));
        assert!(body.contains("Everything you need, nothing you don't"));
        assert!(body.contains("Thoughtful by design"));
        assert!(body.contains("Built for quiet productivity"));
        // FAQ section
        assert!(body.contains("Frequently Asked Questions"));
        assert!(body.contains("<details"));
        assert!(body.contains("<summary><span class=\"faq-chevron\""));
        assert!(body.contains("</span>Where is my data stored?</summary>"));
        assert!(body.contains("stored on your device"));
        assert!(body.contains("Your reminders. One at a time. In order. At your pace."));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-main.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-settings.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-shot-swipe.jpg?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-list.png?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-watch-detail.png?v="#));
        assert!(body.contains(r#"<img src="/static/singlethread-icon.png?v="#));
        assert!(body.contains(r#"<a href="/">Home</a>"#));
        assert!(body.contains(r#"<a href="/singlethread" class="active">SingleThread</a>"#));
        // no legacy component classes remain on this page (checked at class-name
        // boundaries so utility substrings like "list-disc" don't false-positive)
        assert!(!body.contains("\"st-"));
        assert!(!body.contains(" st-"));
        assert!(!body.contains("section-heading"));
        assert!(!body.contains("home-columns"));
        // server-rendered wallpaper from the seeded cache row; minijinja
        // escapes `/` in attribute context, browsers decode it back
        assert!(body.contains("url('https:&#x2f;&#x2f;example.com&#x2f;wallpaper.jpg')"));
        // credit line appears with linked photographer name
        assert!(body.contains("Photo by"));
        assert!(body.contains("Wallpaper Photographer"));
        assert!(body.contains(r#"href="https:&#x2f;&#x2f;unsplash.com&#x2f;@test""#));
        assert!(body.contains("on Unsplash"));
        // responsive: wallpaper and credit are hidden on mobile breakpoints
        assert!(body.contains(r#"class="wallpaper hidden md:block""#));
        // Verify the credit div also has the responsive hide classes. We
        // match a unique snippet from the credit element ("bg-black/50") to
        // confirm it's present, and "hidden md:block" to confirm the
        // responsive pattern (the wallpaper div also carries this, which is
        // fine — both elements need the responsive hide).
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
            .get(format!("http://{addr}/singlethread"))
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
            .get(format!("http://{addr}/singlethread"))
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

    #[tokio::test]
    async fn faq_all_questions_appear() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        for item in all_faq_items() {
            // minijinja's HTML autoescape turns `'` into &#x27; in the rendered
            // page, so compare against the escaped form.
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
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        for item in all_faq_items() {
            // minijinja's HTML autoescape turns `'` and `/` into entities
            // (some answers contain apostrophes such as "you're")
            let escaped_answer = html_escape(item.answer);
            assert!(
                body.contains(&escaped_answer),
                "FAQ answer not found in rendered page: {}",
                &item.answer[..item.answer.len().min(60)],
            );
        }
    }

    #[tokio::test]
    async fn faq_section_after_quiet_productivity_before_cta() {
        let addr = start_app().await;
        let client = test_client();
        let res = client
            .get(format!("http://{addr}/singlethread"))
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
        let cta_pos = body.find("Your reminders. One at a time.").expect("CTA");

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
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed");
        assert_eq!(res.status(), StatusCode::OK);
        let body = res.text().await.unwrap();
        // No <script> tags or onclick handlers anywhere in the page
        assert!(!body.contains("<script"));
        assert!(!body.contains("onclick"));
    }

    #[tokio::test]
    async fn faq_summary_has_chevron() {
        let addr = start_app().await;
        let client = test_client();
        let body = client
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");
        // The disclosure chevron (an inline SVG) must sit inside every question's summary,
        // with the question text immediately following the chevron span.
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
            .get(format!("http://{addr}/singlethread"))
            .send()
            .await
            .expect("request failed")
            .text()
            .await
            .expect("body");

        // Every category renders as an h3 heading.
        for category in FAQS {
            let heading = format!("<h3 class=\"heading-subsection\">{}</h3>", category.title);
            assert!(
                body.contains(&heading),
                "FAQ category heading not found: {}",
                category.title,
            );
        }

        // Each category's first item follows its own heading and precedes
        // the next category's heading, proving items sit under their group.
        for (i, category) in FAQS.iter().enumerate() {
            let heading = format!("<h3 class=\"heading-subsection\">{}</h3>", category.title);
            let heading_pos = body.find(&heading).expect("category heading");
            let first_question = html_escape(category.items[0].question);
            let question_pos = body
                .find(&first_question)
                .expect("first question of category");
            assert!(
                heading_pos < question_pos,
                "question '{}' must appear after its category heading '{}",
                category.items[0].question,
                category.title,
            );
            if let Some(next) = FAQS.get(i + 1) {
                let next_heading = format!("<h3 class=\"heading-subsection\">{}</h3>", next.title);
                let next_pos = body.find(&next_heading).expect("next category heading");
                assert!(
                    question_pos < next_pos,
                    "question '{}' must appear before the next category heading '{}",
                    category.items[0].question,
                    next.title,
                );
            }
        }
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
    fn faq_items_count() {
        assert_eq!(FAQS.len(), 2);
        assert_eq!(all_faq_items().len(), 13);
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
