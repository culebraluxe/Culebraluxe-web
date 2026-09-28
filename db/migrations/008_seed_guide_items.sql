-- Seed current CulebraLuxe Island Guide content into guide_item.
-- Generated from the current cleaned guide-page-fixed.tsx.
-- Media is intentionally NOT migrated here.
--
-- Safe to rerun: rows are upserted by unique slug.

BEGIN;

INSERT INTO guide_item (
    slug,
    section,
    name,
    eyebrow,
    subtitle,
    area,
    description,
    note,
    address,
    phone,
    website_url,
    latitude,
    longitude,
    sort_order,
    is_active
)
VALUES
('flamenco-beach', 'beaches', 'Flamenco Beach', 'Beaches', NULL, 'Flamenco', 'Iconic crescent of soft white sand and calm turquoise water, regularly ranked among the world’s best beaches. The painted tanks at the western end are a distinctive landmark.', NULL, 'PR-251, Culebra, PR 00775', NULL, NULL, 18.3293, -65.3179, 10, true),
('zoni-beach', 'beaches', 'Zoni Beach', 'Beaches', NULL, 'East End', 'Long, quieter stretch of white sand on the eastern side of the island with open views toward Cayo Norte and Culebrita.', NULL, 'PR-250 East End, Culebra, PR 00775', NULL, NULL, 18.3188, -65.2532, 20, true),
('tamarindo-beach', 'beaches', 'Tamarindo Beach', 'Beaches', NULL, 'Tamarindo', 'Mix of sand and pebbles with exceptionally clear water. A reliable place to snorkel and often see sea turtles and rays close to shore.', NULL, 'PR-251, Culebra, PR 00775', NULL, NULL, 18.3181, -65.3175, 30, true),
('carlos-rosario-beach', 'beaches', 'Carlos Rosario Beach', 'Beaches', NULL, 'Luis Peña Channel', 'Reached by a short trail from Flamenco. One of the island’s strongest snorkeling spots thanks to healthy coral and the protected waters of the Luis Peña Channel.', NULL, 'Trail from Flamenco Beach, Culebra, PR 00775', NULL, NULL, 18.3256, -65.3289, 40, true),
('melones-beach', 'beaches', 'Melones Beach', 'Beaches', NULL, 'Melones', 'West-facing cove with calm water, easy snorkeling, and consistent sunset views. A convenient and popular local choice.', NULL, 'Camino Melones, Culebra, PR 00775', NULL, NULL, 18.3031, -65.3115, 50, true),
('brava-beach', 'beaches', 'Brava Beach', 'Beaches', NULL, 'North Coast', 'Remote north-coast beach reached by a scenic inland trail. Wilder Atlantic character and usually very quiet.', NULL, 'Brava Trail off PR-250, Culebra, PR 00775', NULL, NULL, 18.3242, -65.2819, 60, true),
('resaca-beach', 'beaches', 'Resaca Beach', 'Beaches', NULL, 'North Coast', 'Secluded, windswept beach reached by a steep hike through boulder forest. Beautiful and rarely crowded; swimming is not recommended due to strong currents.', NULL, 'Trail off PR-250 near the airport, Culebra, PR 00775', NULL, NULL, 18.3309, -65.3001, 70, true),
('punta-soldado', 'beaches', 'Punta Soldado', 'Beaches', NULL, 'Southwest Tip', 'Rocky southern point with clear water and good snorkeling along the reef. Less visited and often quiet, with views across the channel.', NULL, 'Punta Soldado, Culebra, PR 00775', NULL, NULL, 18.293, -65.302, 80, true),
('datiles-beach', 'beaches', 'Dátiles Beach', 'Beaches', NULL, 'Southeast', 'Shallow, calm cove close to town. Gentle water makes it a good option for families, kayaking, and easy floating.', NULL, 'Near Dewey / Playa Sardinas area, Culebra, PR 00775', NULL, NULL, 18.2994, -65.3011, 90, true),
('snorkeling', 'water', 'Snorkeling', 'Water', NULL, 'Tamarindo · Melones · Carlos Rosario', 'Explore shallow reefs, seagrass beds and clear coastal water shared by turtles, rays and colorful reef life.', NULL, NULL, NULL, NULL, NULL, NULL, 10, true),
('diving', 'water', 'Diving', 'Water', NULL, 'Offshore reefs', 'Boat diving opens access to coral formations, reef walls and less-visited underwater sites surrounding the island.', NULL, NULL, NULL, NULL, NULL, NULL, 20, true),
('boat-charters', 'water', 'Boat Charters', 'Water', NULL, 'Culebra & surrounding cays', 'Private and small-group trips offer a relaxed way to reach Culebrita, Luis Peña and the surrounding cays.', NULL, NULL, NULL, NULL, NULL, NULL, 30, true),
('sailing', 'water', 'Sailing', 'Water', NULL, 'Culebra waters', 'Wind-powered excursions and sunset sails reveal the island from the water and connect its many bays and cays.', NULL, NULL, NULL, NULL, NULL, NULL, 40, true),
('kayaking', 'water', 'Kayaking', 'Water', NULL, 'Protected bays & coves', 'Paddle calm coves and protected shoreline at a slower pace, with easy access to shallow marine habitats.', NULL, NULL, NULL, NULL, NULL, NULL, 50, true),
('exploring-the-cays', 'water', 'Exploring the Cays', 'Water', NULL, 'Culebrita · Luis Peña · surrounding cays', 'Discover the smaller islands surrounding Culebra, from Culebrita and its natural pools to the protected waters around Luis Peña.', NULL, NULL, NULL, NULL, NULL, NULL, 60, true),
('wildlife-refuge', 'wildlife-land', 'Culebra National Wildlife Refuge', 'Wildlife & Land', 'Protected since 1909', 'Culebra & surrounding cays', 'One of the Caribbean’s oldest federal wildlife refuges, protecting important coastal habitat and offshore cays.', NULL, NULL, '+17877420115', 'https://www.fws.gov/refuge/culebra', 18.3089, -65.2911, 10, true),
('sea-turtles', 'wildlife-land', 'Sea Turtles', 'Wildlife & Land', NULL, 'Beaches & coastal waters', 'Culebra provides important feeding and nesting habitat for protected marine turtles, including green, hawksbill and leatherback turtles.', NULL, NULL, NULL, NULL, NULL, NULL, 20, true),
('birding', 'wildlife-land', 'Birding', 'Wildlife & Land', NULL, 'Refuge lands & offshore cays', 'Protected cays and coastal habitat support resident, migratory and nesting seabirds throughout the archipelago.', NULL, NULL, NULL, NULL, NULL, NULL, 30, true),
('hiking-and-trails', 'wildlife-land', 'Hiking & Trails', 'Wildlife & Land', NULL, 'Resaca · Brava · island interior', 'Footpaths cross Culebra’s dry landscape and lead to remote beaches, high points and less-traveled corners of the island.', NULL, NULL, NULL, NULL, NULL, NULL, 40, true),
('dry-forest', 'wildlife-land', 'Dry Forest', 'Wildlife & Land', NULL, 'Island interior', 'Cacti, scrub and drought-adapted vegetation form a distinctive subtropical dry-forest landscape beyond the shoreline.', NULL, NULL, NULL, NULL, NULL, NULL, 50, true),
('conservation', 'wildlife-land', 'Conservation', 'Wildlife & Land', 'Community stewardship', 'Island-wide', 'Local organizations and residents work alongside public agencies to protect reefs, shorelines, wildlife and fragile coastal habitats.', NULL, NULL, NULL, 'https://www.coralations.org', NULL, NULL, 60, true),
('pan-deli', 'coffee-casual', 'Pan Deli', 'Eat & Drink', 'Breakfast & coffee', 'Dewey', 'Busy bakery and café for morning coffee, breakfast sandwiches, and pastries. A practical downtown stop before heading out.', NULL, 'Calle Pedro Márquez #17, Culebra, PR 00775', '+17877423311', NULL, 18.3029, -65.3007, 10, true),
('blac-flamingo-coffee', 'coffee-casual', 'Blac Flamingo Coffee', 'Eat & Drink', 'Specialty coffee & breakfast', 'La Romana', 'Specialty coffee and carefully made breakfast in an eclectic space with indoor and outdoor seating.', NULL, 'PR-250 C-19, Sector La Romana, Culebra, PR 00775', '+19393800782', NULL, 18.3105, -65.295, 20, true),
('culebra-coffee', 'coffee-casual', 'Culebra Coffee', 'Eat & Drink', 'Puerto Rican coffee & café', 'Dewey', 'Focused on Puerto Rican coffee along with breakfast sandwiches, light meals, and smoothies in the center of town.', NULL, '2 Calle Pedro Márquez, Culebra, PR 00775', '+13022200773', NULL, 18.3017, -65.3024, 30, true),
('latitud-perfecta-cafe', 'coffee-casual', 'Latitud Perfecta Café', 'Eat & Drink', 'Casual island fare', 'Dewey', 'Straightforward town café for burgers, simple plates, and drinks after a day outside.', NULL, 'Calle Pedro Márquez, Culebra, PR 00775', '+17877420104', NULL, 18.3015, -65.3028, 40, true),
('kiosk2', 'coffee-casual', 'Kiosk #2', 'Eat & Drink', 'Beach kiosk', 'Flamenco Beach', 'Beachside kiosk steps from the sand at Flamenco, serving basic food and cold drinks.', NULL, 'Flamenco Beach, Culebra, PR 00775', '+17877420111', NULL, 18.3281, -65.3181, 50, true),
('rolls-of-heaven', 'coffee-casual', 'Rolls of Heaven', 'Eat & Drink', 'Rolled ice cream & light bites', 'Dewey', 'Rolled ice cream and a few light snacks or smoothies. Easy dessert stop in Dewey.', NULL, '26 Calle Pedro Márquez, Culebra, PR 00775', '+17873977728', NULL, 18.3025, -65.301, 60, true),
('dinghy-dock', 'dining', 'Dinghy Dock', 'Eat & Drink', 'Waterfront dining', 'Fulladoza', 'Longtime waterfront favorite with outdoor seating right on the harbor. Known for seafood, casual plates, and the easygoing dockside atmosphere.', NULL, '372 Calle Fulladoza, Culebra, PR 00775', '+17877420233', 'http://www.dinghydock.com', 18.3006, -65.2994, 10, true),
('mamacitas', 'dining', 'Mamacita''s', 'Eat & Drink', 'Canal-side dining', 'Dewey', 'Canal-side restaurant and bar serving Caribbean and Puerto Rican dishes with a lively, local feel. A reliable stop for dinner and drinks in town.', NULL, '64 Calle Castelar, Culebra, PR 00775', '+17877420090', 'https://www.eatatmamacitas.com/', 18.3012, -65.3019, 20, true),
('zacos-tacos', 'dining', 'Zaco''s Tacos', 'Eat & Drink', 'Mexican & Caribbean tacos', 'Dewey', 'Colorful taqueria in the middle of Dewey offering creative tacos, strong drinks, and a fun, casual energy.', NULL, '21 Calle Pedro Márquez, Culebra, PR 00775', '+17877420243', 'https://www.zacostacos.com', 18.3032, -65.3003, 30, true),
('la-jibara', 'dining', 'La Jíbara – Pizzería Creativa', 'Eat & Drink', 'Sourdough pizza', 'Dewey · Ferry plaza', 'Sourdough pizza made with local ingredients, including cheese from island cows. Popular evening option near the ferry dock.', NULL, 'Culebra Public Plaza (next to ferry terminal), Calle Pedro Márquez, Culebra, PR 00775', '+17873984050', 'http://www.lajibarapizzeriacreativa.com', 18.3017, -65.3026, 40, true),
('heathers-pizza', 'dining', 'Heather''s Pizza', 'Eat & Drink', 'Pizza & casual Italian', 'Dewey', 'Established Dewey spot for pizza, calzones, and straightforward Italian-American comfort food.', NULL, '14 Calle Pedro Márquez, Culebra, PR 00775', '+17877423175', NULL, 18.3018, -65.302, 50, true),
('susies', 'dining', 'Susie''s Restaurant', 'Eat & Drink', 'Puerto Rican–Asian fusion', 'Las Delicias', 'Hillside restaurant blending Puerto Rican and Asian flavors, served in a quiet outdoor garden setting.', NULL, 'PR-250, Sector Las Delicias, Culebra, PR 00775', '+17873407058', 'https://www.susiesculebra.com', 18.31, -65.29, 60, true),
('la-cocina-del-navegante', 'dining', 'La Cocina del Navegante', 'Eat & Drink', 'Puerto Rican seafood with bay views', 'Dewey', 'Puerto Rican cooking with views over the bay. Fresh seafood and classic island plates in a relaxed waterfront location.', NULL, 'Calle Pedro Márquez, Culebra, PR 00775', '+17878067586', NULL, 18.3034, -65.2997, 70, true),
('harspoons', 'dining', 'Harspoon''s', 'Eat & Drink', 'Seafood & creative island cooking', 'Barriada Clark', 'Well-regarded for fresh seafood and creative preparations in a comfortable, low-key space. Reservations are a good idea.', NULL, '40 Calle Luis Muñoz Marín / Barriada Clark, Culebra, PR 00775', '+17872855034', NULL, 18.305, -65.298, 80, true),
('tikis-grill', 'dining', 'Tiki''s Grill', 'Eat & Drink', 'Burgers & casual island fare', 'Dewey · Near ferry', 'Casual spot near the ferry for burgers, fish sandwiches, wraps, and cold drinks. Convenient before or after the boat ride.', NULL, 'Calle Pedro Márquez (near Banco Popular / ferry terminal), Culebra, PR 00775', '+17877420241', NULL, 18.302, -65.3015, 90, true),
('caracoles', 'dining', 'Caracoles Restaurant', 'Eat & Drink', 'Seafood & Puerto Rican classics', 'Playa Sardinas / waterfront', 'Waterfront place serving generous portions of seafood and traditional Puerto Rican dishes. Worth confirming current hours.', NULL, 'Calle Villa Pesquera, Sector Playa Sardinas II, Culebra, PR 00775', '+17877421507', NULL, 18.3005, -65.3, 100, true),
('culebra-airport', 'getting-here', 'Culebra Airport (CPX)', 'Getting Here', 'Benjamín Rivera Noriega Airport', 'Culebra', 'Culebra’s small airport handles scheduled regional flights, private aviation and charter service.', NULL, 'Carretera 251, Culebra, PR 00775', NULL, NULL, 18.313, -65.3039, 10, true),
('air-flamenco', 'getting-here', 'Air Flamenco', 'Getting Here', 'Regional air service', 'San Juan · Ceiba · Culebra', 'Regional commuter service connecting Culebra with airports on Puerto Rico’s main island.', NULL, NULL, '+17877241818', 'https://www.booking.flyairflamenco.com/', NULL, NULL, 20, true),
('cape-air', 'getting-here', 'Cape Air', 'Getting Here', 'San Juan air service', 'San Juan · Culebra', 'Scheduled regional flights connect San Juan with Culebra’s Benjamín Rivera Noriega Airport.', NULL, NULL, NULL, 'https://www.capeair.com/', NULL, NULL, 30, true),
('ceiba-ferry-terminal', 'getting-here', 'Ceiba Ferry Terminal', 'Getting Here', 'Mainland departure point', 'Ceiba', 'The mainland terminal for passenger ferry service to Culebra, located on the former Roosevelt Roads property.', NULL, 'Roosevelt Roads, Ceiba, PR 00735', NULL, NULL, 18.2439, -65.6022, 40, true),
('puerto-rico-ferry', 'getting-here', 'Puerto Rico Ferry', 'Getting Here', 'Ceiba ↔ Culebra', 'Ceiba · Culebra', 'Official passenger ferry service connecting Ceiba on Puerto Rico’s main island with Culebra.', 'Schedules, ticket availability and check-in requirements should always be confirmed before travel.', NULL, NULL, 'https://www.puertoricoferry.com/', NULL, NULL, 50, true),
('san-juan-connection', 'getting-here', 'From San Juan', 'Getting Here', 'Choose air or connect through Ceiba', 'San Juan → Culebra', 'From metropolitan San Juan, visitors can fly directly to Culebra or travel east to Ceiba and continue by ferry.', 'Flying is the most direct option. Ferry travelers first travel overland from San Juan to Ceiba.', NULL, NULL, NULL, NULL, NULL, 60, true),
('carlos-jeep-rentals', 'getting-around', 'Carlos Jeep Rentals', 'Getting Around', 'Carlos Jeep Rentals', 'Near the airport', 'Jeeps are a popular choice for Culebra’s hills, beach access roads and relaxed open-air island travel.', NULL, 'PR-250, Culebra, PR 00775', '+17877423514', 'https://www.carlosjeeprental.com', 18.3119, -65.3031, 10, true),
('jerry-jeep-rentals', 'getting-around', 'Jerrys Jeep Rentals', 'Getting Around', 'Jerry''s Jeeps & Carts', 'Near the airport', 'Open-air carts provide a casual way to move between town, beaches and accommodations around the island.', NULL, 'PR-250, Culebra, PR 00775', '+17877420587', NULL, 18.3122, -65.3039, 20, true),
('chocosbroncorental', 'getting-around', 'Chocos Bronco Rentals', 'Getting Around', 'Chocos Bronco Rental', 'Culebra', 'Chocos Bronco Rentals for visitors who prefer an Broncos.', NULL, 'PR-250, Culebra, PR 00775', '+17877423537', NULL, 18.3121, -65.3028, 30, true),
('island-taxi', 'getting-around', 'Taxis & Públicos', 'Getting Around', NULL, 'Ferry dock · Airport · Flamenco', 'Local taxi vans and drivers connect the ferry dock, airport, Flamenco Beach and other common island destinations.', NULL, NULL, NULL, NULL, NULL, NULL, 40, true),
('supermercado-milka', 'essentials', 'Supermercado Milka', 'Essentials', 'Econo', 'Dewey', 'Full-service grocery shopping for food, household supplies and the basics of everyday island life.', NULL, 'Calle William Font, Culebra, PR 00775', '+17877420528', NULL, 18.3022, -65.3025, 10, true),
('supermercado-costa-del-sol', 'essentials', 'Supermercado Costa Del Sol', 'Essentials', 'Econo', 'Dewey', 'Full-service grocery shopping for food, household supplies and the basics of everyday island life.', NULL, 'Calle William Font, Culebra, PR 00775', '+17877420528', NULL, 18.3022, -65.3025, 20, true),
('supermercado-mayras', 'essentials', 'Supermercado Mayras', 'Essentials', 'Farmacia Culebra', 'Dewey', ' A local grocery and pharmacy for everyday needs, including food, household supplies and personal care items', NULL, 'Calle Pedro Márquez, Culebra, PR 00775', '+17877420710', NULL, 18.3015, -65.3027, 30, true),
('emergency-medical-services', 'essentials', 'Emergency And Medical Services', 'Essentials', 'Culebra health services', 'Culebra', 'Local emergency and medical services provide basic care and urgent evaluation, with more advanced treatment available on Puerto Rico’s main island.', NULL, 'Calle Unión, Culebra, PR 00775', '+17877420110', NULL, 18.3061, -65.3014, 40, true),
('atm-banking', 'essentials', 'ATM / Banking', 'Essentials', 'Cash access', 'Dewey', 'Cash remains useful around the island, particularly for some transportation, kiosks and smaller local businesses.', NULL, NULL, NULL, NULL, NULL, NULL, 50, true),
('ferreteria-gonzalez', 'essentials', 'Ferreteria Gonzalez', 'Essentials', NULL, 'Culebra', 'Ferreteria Gonzalez Island hardware and household suppliers are useful for tools, repairs, marine needs and the realities of maintaining a home here.', NULL, NULL, NULL, NULL, NULL, NULL, 60, true),
('post-office', 'essentials', 'Post Office', 'Essentials', 'USPS Culebra', 'Dewey', 'USPS service supports island mail, packages and post-office boxes as part of everyday life in Culebra.', NULL, 'Culebra, PR 00775', NULL, 'https://www.usps.com/', NULL, NULL, 70, true),
('early-inhabitants', 'island-story', 'Early Inhabitants', 'Island Story', 'Before European colonization', 'Culebra', 'Indigenous peoples Taino Arawak lived throughout the northeastern Caribbean long before European colonization, using these islands and waters for settlement, fishing and navigation.', 'Era: Pre-colonial', NULL, NULL, NULL, NULL, NULL, 10, true),
('spanish-period', 'island-story', 'Spanish Period', 'Island Story', 'Colonial era', 'Culebra', 'Culebra remained within the Spanish Caribbean world for centuries before formal settlement accelerated in the nineteenth century.', 'Era: Spanish rule', NULL, NULL, NULL, NULL, NULL, 20, true),
('island-life', 'island-story', 'Pirate Life', 'Island Story', 'An island community takes shape', 'Dewey', 'Fishing, farming, maritime activity and close community ties shaped life as permanent settlement developed.', 'Era: Late 19th and early 20th centuries', NULL, NULL, NULL, NULL, NULL, 30, true),
('us-navy-presence', 'island-story', 'U.S. Navy Presence', 'Island Story', 'Military era', 'Culebra', 'Large parts of Culebra and its surrounding waters became associated with U.S. military training, profoundly affecting the island and its residents.', 'Era: 20th century', NULL, NULL, NULL, NULL, NULL, 40, true),
('community-resistance', 'island-story', 'Community Resistance', 'Island Story', 'Island activism', 'Culebra', 'Residents and supporters organized sustained civil action against military exercises, making the struggle for Culebra nationally significant.', 'Era: 1970s', NULL, NULL, NULL, NULL, NULL, 50, true),
('navy-departure', 'island-story', 'A New Chapter', 'Island Story', 'Transition', 'Culebra', 'The end of Navy exercises on Culebra in 1975 marked a defining transition for the island and its relationship with its protected landscape.', 'Era: 1975', NULL, NULL, NULL, NULL, NULL, 60, true),
('conservation-legacy', 'island-story', 'Conservation Legacy', 'Island Story', 'Protection & stewardship', 'Culebra', 'Refuge lands, marine protection and community stewardship became central to preserving the landscapes that define modern Culebra.', 'Era: Continuing legacy', NULL, NULL, NULL, NULL, NULL, 70, true),
('culebra-today', 'island-story', 'Culebra Today', 'Island Story', 'An island intentionally apart', 'Culebra', 'Culebra remains small, lightly developed and deeply shaped by the tension between access, community, conservation and change.', 'Era: Present', NULL, NULL, NULL, NULL, NULL, 80, true)
ON CONFLICT (slug) DO UPDATE SET
    section = EXCLUDED.section,
    name = EXCLUDED.name,
    eyebrow = EXCLUDED.eyebrow,
    subtitle = EXCLUDED.subtitle,
    area = EXCLUDED.area,
    description = EXCLUDED.description,
    note = EXCLUDED.note,
    address = EXCLUDED.address,
    phone = EXCLUDED.phone,
    website_url = EXCLUDED.website_url,
    latitude = EXCLUDED.latitude,
    longitude = EXCLUDED.longitude,
    sort_order = EXCLUDED.sort_order,
    is_active = EXCLUDED.is_active,
    updated_at = now();

COMMIT;

-- Verification
SELECT section, count(*) AS item_count
FROM guide_item
WHERE is_active = true
GROUP BY section
ORDER BY
    CASE section
        WHEN 'beaches' THEN 1
        WHEN 'water' THEN 2
        WHEN 'wildlife-land' THEN 3
        WHEN 'coffee-casual' THEN 4
        WHEN 'dining' THEN 5
        WHEN 'getting-here' THEN 6
        WHEN 'getting-around' THEN 7
        WHEN 'essentials' THEN 8
        WHEN 'island-story' THEN 9
        ELSE 99
    END,
    section;

SELECT count(*) AS total_guide_items
FROM guide_item
WHERE is_active = true;
