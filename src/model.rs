//! Content model. New pages/rails/cards are added here without touching the
//! rendering or navigation layers.

/// The single sample clip every card plays (royalty-free test asset).
pub const SAMPLE_VIDEO_URL: &str =
    "https://commondatastorage.googleapis.com/gtv-videos-bucket/sample/BigBuckBunny.mp4";

/// One selectable tile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    pub id: u32,
    pub title: String,
    /// Poster art URL (loaded asynchronously by the renderer).
    pub image_url: String,
}

/// A horizontal carousel of cards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rail {
    pub title: &'static str,
    pub cards: Vec<Card>,
}

/// The whole browse page: an ordered list of rails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub rails: Vec<Rail>,
}

impl Catalog {
    /// Demo content: 3 rails × 10 cards, poster art from picsum with fixed
    /// per-card seeds so the same image is stable across reloads.
    pub fn sample() -> Self {
        const RAIL_TITLES: [&str; 3] = ["Trending Now", "Popular on Leanback", "Continue Watching"];
        // 16:9 poster art; keep the request small, the renderer stretches to card size.
        const ART_W: u32 = 480;
        const ART_H: u32 = 270;
        const PER_RAIL: usize = 10;

        let mut rails = Vec::with_capacity(RAIL_TITLES.len());
        let mut id = 0u32;
        for (r, &title) in RAIL_TITLES.iter().enumerate() {
            let mut cards = Vec::with_capacity(PER_RAIL);
            for c in 0..PER_RAIL {
                let seed = format!("lb-r{r}-c{c}");
                cards.push(Card {
                    id,
                    title: format!("{title} {}", c + 1),
                    image_url: format!("https://picsum.photos/seed/{seed}/{ART_W}/{ART_H}"),
                });
                id += 1;
            }
            rails.push(Rail { title, cards });
        }
        Self { rails }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_has_expected_shape() {
        let cat = Catalog::sample();
        assert_eq!(cat.rails.len(), 3);
        for rail in &cat.rails {
            assert_eq!(rail.cards.len(), 10);
        }
    }

    #[test]
    fn card_ids_are_unique() {
        let cat = Catalog::sample();
        let mut ids: Vec<u32> = cat.rails.iter().flat_map(|r| r.cards.iter().map(|c| c.id)).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
