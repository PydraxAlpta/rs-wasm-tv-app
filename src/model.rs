//! Content model. New pages/rails/cards are added here without touching the
//! rendering or navigation layers.

/// The single sample clip every card plays (royalty-free test asset).
pub const SAMPLE_VIDEO_URL: &str = "https://samplelib.com/mp4/sample-15s-720p.mp4";

/// Demo titles reused across rails (card labels cycle through this list).
const SAMPLE_TITLES: [&str; 50] = [
    "Night Harbor",
    "Glass Orchard",
    "Silent Cascade",
    "Amber Circuit",
    "Northern Drift",
    "Paper Lanterns",
    "Iron Meadow",
    "Velvet Signal",
    "Copper Sky",
    "Hidden Current",
    "Last Station",
    "Bright Asylum",
    "Fable Ridge",
    "Quiet Voltage",
    "Marble Tide",
    "Broken Compass",
    "Solar Archive",
    "Winter Protocol",
    "Echo Canyon",
    "Silver Orchid",
    "Dust Ballet",
    "Cedar Frequency",
    "Midnight Ledger",
    "Pale Empire",
    "River Cipher",
    "Static Garden",
    "Hollow Crown",
    "Neon Prairie",
    "Forgotten Atlas",
    "Crimson Relay",
    "Soft Apocalypse",
    "Blue Workshop",
    "Ancient Modem",
    "Lunar Kitchen",
    "Ghost Frequency",
    "Ivory Engine",
    "Parallel Harbor",
    "Saffron Drift",
    "Obsidian Choir",
    "Tidal Archive",
    "Emerald Static",
    "Forgotten Pier",
    "Carbon Sonata",
    "White Noise Farm",
    "Azure Fracture",
    "Candle Network",
    "Moss Terminal",
    "Phantom Gallery",
    "Golden Outage",
    "Quiet Firewall",
];

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
    pub title: String,
    pub cards: Vec<Card>,
}

/// Full-width hero slide above the rails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BannerSlide {
    pub title: String,
    pub image_url: String,
}

/// The whole browse page: hero banners + an ordered list of rails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    pub banners: Vec<BannerSlide>,
    pub rails: Vec<Rail>,
}

impl Catalog {
    /// Demo content: hero banners + 20 rails × 20 cards (UI lazy-reveals rails
    /// in batches). Poster art from picsum with fixed seeds so images stay
    /// stable across reloads.
    pub fn sample() -> Self {
        const BANNER_COUNT: usize = 5;
        const RAIL_COUNT: usize = 20;
        const PER_RAIL: usize = 20;
        // Wide hero art; renderer stretches to the banner rect.
        const BANNER_W: u32 = 1920;
        const BANNER_H: u32 = 600;
        // Match on-screen card size (Metrics::tv card_w/card_h) to cut texture cost.
        const ART_W: u32 = 200;
        const ART_H: u32 = 300;

        let mut banners = Vec::with_capacity(BANNER_COUNT);
        for b in 0..BANNER_COUNT {
            let seed = format!("lb-banner-{b}");
            banners.push(BannerSlide {
                title: SAMPLE_TITLES[b % SAMPLE_TITLES.len()].to_string(),
                image_url: format!("https://picsum.photos/seed/{seed}/{BANNER_W}/{BANNER_H}"),
            });
        }

        let mut rails = Vec::with_capacity(RAIL_COUNT);
        let mut id = 0u32;
        for r in 0..RAIL_COUNT {
            let title = format!("Collection {}", r + 1);
            let mut cards = Vec::with_capacity(PER_RAIL);
            for c in 0..PER_RAIL {
                let seed = format!("lb-r{r}-c{c}");
                let title_i = (r * PER_RAIL + c) % SAMPLE_TITLES.len();
                cards.push(Card {
                    id,
                    title: SAMPLE_TITLES[title_i].to_string(),
                    image_url: format!("https://picsum.photos/seed/{seed}/{ART_W}/{ART_H}"),
                });
                id += 1;
            }
            rails.push(Rail { title, cards });
        }
        Self { banners, rails }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_has_expected_shape() {
        let cat = Catalog::sample();
        assert_eq!(cat.banners.len(), 5);
        assert_eq!(cat.rails.len(), 20);
        for rail in &cat.rails {
            assert_eq!(rail.cards.len(), 20);
        }
    }

    #[test]
    fn sample_card_titles_come_from_fixed_set() {
        let cat = Catalog::sample();
        let titles: std::collections::HashSet<&str> = SAMPLE_TITLES.iter().copied().collect();
        for rail in &cat.rails {
            for card in &rail.cards {
                assert!(
                    titles.contains(card.title.as_str()),
                    "unexpected title {}",
                    card.title
                );
            }
        }
        assert_eq!(cat.rails[0].cards[0].title, "Night Harbor");
        assert_eq!(cat.rails[0].cards[1].title, "Glass Orchard");
    }

    #[test]
    fn card_ids_are_unique() {
        let cat = Catalog::sample();
        let mut ids: Vec<u32> = cat
            .rails
            .iter()
            .flat_map(|r| r.cards.iter().map(|c| c.id))
            .collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
    }
}
