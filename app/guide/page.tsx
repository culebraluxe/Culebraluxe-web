import type { Metadata } from 'next'
import Link from 'next/link'

import { PageHero } from '@/components/page-hero'
import { Reveal } from '@/components/reveal'
import { SiteFooter } from '@/components/site-footer'
import { SiteHeader } from '@/components/site-header'

export const metadata: Metadata = {
  title: 'Island Guide — CulebraLuxe',
  description:
    'A quiet guide to life on Culebra — its beaches, marine reserves, and the unhurried rhythm that rewards those who arrive attentively.',
}

const GUIDE_SECTIONS = [
  {
    id: 'beaches',
    number: '01',
    title: 'BEACHES',
    headline: 'The edges of the island.',
    description:
      'From world-famous shores to quiet hidden coves, every beach in Culebra has its own character.',
  },
  {
    id: 'water',
    number: '02',
    title: 'WATER',
    headline: 'Our greatest treasure.',
    description:
      'Snorkel, dive, sail and explore the cays. The water here is unbelievably clear—and full of life.',
  },
  {
    id: 'wildlife-land',
    number: '03',
    title: 'WILDLIFE & LAND',
    headline: 'The wild island.',
    description:
      'Refuge lands, native wildlife, trails and protected habitats reveal another side of Culebra.',
  },
  {
    id: 'coffee-casual',
    number: '04',
    title: 'COFFEE & CASUAL',
    headline: 'Easy mornings, easy meals.',
    description:
      'Coffee, breakfast, beach food and relaxed island stops for the everyday rhythm of Culebra.',
  },
  {
    id: 'dining',
    number: '05',
    title: 'DINING',
    headline: 'Island evenings, done well.',
    description:
      'From waterfront tables to local favorites, these are the places worth lingering over dinner.',
  },
  {
    id: 'getting-here',
    number: '06',
    title: 'GETTING HERE',
    headline: 'Two ways in. Worth every mile.',
    description:
      'Fly or take the ferry—both are part of the adventure.',
  },
  {
    id: 'getting-around',
    number: '07',
    title: 'GETTING AROUND',
    headline: 'Small island. Easy to explore.',
    description:
      'Rent a car, Jeep or golf cart and you’re on island time.',
  },
  {
    id: 'essentials',
    number: '08',
    title: 'ISLAND ESSENTIALS',
    headline: 'Everything you need, right here.',
    description:
      'Local services and essentials that make everyday island life easier.',
  },
  {
    id: 'island-story',
    number: '09',
    title: 'ISLAND STORY',
    headline: 'How Culebra became Culebra.',
    description:
      'A small island with a layered history of settlement, military presence, community resistance and conservation.',
  },
]
export const culebraGuideItems: GuideItem[] = [
  // ==========================================
  // 01 BEACHES
  // ==========================================

  {
    id: 'flamenco-beach',
    sectionId: 'beaches',
    name: 'Flamenco Beach',
    image: {
      src: '/images/guide/beaches/flamenco-beach.jpg',
      alt: 'Flamenco Beach in Culebra, Puerto Rico',
    },
    eyebrow: 'Beaches',
    area: 'Flamenco',
    address: 'PR-251, Culebra, PR 00775',
    latitude: 18.3293,
    longitude: -65.3179,
    description:
      'World-famous white sand crescent beach with calm turquoise waters and iconic painted Sherman tanks.',
  },

  {
    id: 'zoni-beach',
    sectionId: 'beaches',
    name: 'Zoni Beach',
    image: {
      src: '/images/guide/beaches/zoni-beach.jpg',
      alt: 'Zoni Beach in Culebra, Puerto Rico',
    },
    eyebrow: 'Beaches',
    area: 'Fraile',
    address: 'PR-250 East End, Culebra, PR 00775',
    latitude: 18.3188,
    longitude: -65.2532,
    description:
      'Quiet, long stretch of white sand on the east side offering views of Cayo Norte and Culebrita.',
  },

  {
    id: 'tamarindo-beach',
    sectionId: 'beaches',
    name: 'Tamarindo Beach',
    image: {
      src: '/images/guide/beaches/tamarindo-beach.jpg',
      alt: 'Tamarindo Beach in Culebra, Puerto Rico',
    },
    eyebrow: 'Beaches',
    area: 'Tamarindo',
    address: 'PR-251, Culebra, PR 00775',
    latitude: 18.3181,
    longitude: -65.3175,
    description:
      'Pebble and sand beach known for clear-water snorkeling and frequent encounters with sea turtles and rays.',
  },

  {
    id: 'carlos-rosario-beach',
    sectionId: 'beaches',
    name: 'Carlos Rosario Beach',
    image: {
      src: '/images/guide/beaches/carlos-rosario-beach.jpg',
      alt: 'Carlos Rosario Beach in Culebra, Puerto Rico',
    },
    eyebrow: 'Beaches',
    area: 'Luis Peña Channel',
    address: 'Flamenco Trail, Culebra, PR 00775',
    latitude: 18.3256,
    longitude: -65.3289,
    description:
      'Secluded shoreline reached by trail from Flamenco, with excellent access to coral reef snorkeling.',
  },

  {
    id: 'melones-beach',
    sectionId: 'beaches',
    name: 'Melones Beach',
    image: {
      src: '/images/guide/beaches/melones-beach.jpg',
      alt: 'Melones Beach in Culebra, Puerto Rico',
    },
    eyebrow: 'Beaches',
    area: 'Melones',
    address: 'Camino Melones, Culebra, PR 00775',
    latitude: 18.3031,
    longitude: -65.3115,
    description:
      'West-facing calm cove with abundant marine life, casual snorkeling and beautiful sunset views.',
  },

  {
    id: 'brava-beach',
    sectionId: 'beaches',
    name: 'Brava Beach',
    image: {
      src: '/images/guide/beaches/brava-beach.jpg',
      alt: 'Brava Beach on the north coast of Culebra',
    },
    eyebrow: 'Beaches',
    area: 'North Coast',
    address: 'Brava Trail off PR-250, Culebra, PR 00775',
    latitude: 18.3242,
    longitude: -65.2819,
    description:
      'Remote north-coast beach with a wilder Atlantic character, reached by a scenic trail through the island interior.',
  },

  // ==========================================
  // 02 WATER
  // ==========================================

  {
    id: 'snorkeling',
    sectionId: 'water',
    name: 'Snorkeling',
    image: {
       src: '/images/guide/water/snorkeling.jpg',
      alt: 'Snorkeling in the clear waters around Culebra',
    },
    eyebrow: 'Water',
    area: 'Tamarindo · Melones · Carlos Rosario',
    description:
      'Explore shallow reefs, seagrass beds and clear coastal water shared by turtles, rays and colorful reef life.',
  },

  {
    id: 'diving',
    sectionId: 'water',
    name: 'Diving',
    image: {
      src: '/images/guide/water/scuba-diving.jpg',
      alt: 'Scuba diving in Culebra',
    },
    eyebrow: 'Water',
    area: 'Offshore reefs',
    description:
      'Boat diving opens access to coral formations, reef walls and less-visited underwater sites surrounding the island.',
  },

  {
    id: 'boat-charters',
    sectionId: 'water',
    name: 'Boat Charters',
    image: {
      src: '/images/guide/water/boat-charters3.jpg',
      alt: 'Private boat charter in Culebra',
    },
    eyebrow: 'Water',
    area: 'Culebra & surrounding cays',
    description:
      'Private and small-group trips offer a relaxed way to reach Culebrita, Luis Peña and the surrounding cays.',
  },

  {
    id: 'sailing',
    sectionId: 'water',
    name: 'Sailing',
    image: {
      src: '/images/guide/water/sailing.jpg',
      alt: 'Sailing around Culebra',
    },
    eyebrow: 'Water',
    area: 'Culebra waters',
    description:
      'Wind-powered excursions and sunset sails reveal the island from the water and connect its many bays and cays.',
  },

  {
    id: 'kayaking',
    sectionId: 'water',
    name: 'Kayaking',
    image: {
      src: '/images/guide/water/kayaking.jpg',
      alt: 'Kayaking along the coast of Culebra',
    },
    eyebrow: 'Water',
    area: 'Protected bays & coves',
    description:
      'Paddle calm coves and protected shoreline at a slower pace, with easy access to shallow marine habitats.',
  },

  {
    id: 'exploring-the-cays',
    sectionId: 'water',
    name: 'Exploring the Cays',
    image: {
      src: '/images/guide/water/exploring-the-cays.jpg',
      alt: 'Culebrita and the offshore cays surrounding Culebra',
    },
    eyebrow: 'Water',
    area: 'Culebrita · Luis Peña · surrounding cays',
    description:
      'Discover the smaller islands surrounding Culebra, from Culebrita and its natural pools to the protected waters around Luis Peña.',
  },

  // ==========================================
  // 03 WILDLIFE & LAND
  // ==========================================

  {
    id: 'wildlife-refuge',
    sectionId: 'wildlife-land',
    name: 'Culebra National Wildlife Refuge',
    image: {
      src: '/images/guide/wildlife/wildlife.jpg',
      alt: 'Culebra National Wildlife Refuge',
    },
    eyebrow: 'Wildlife & Land',
    subtitle: 'Protected since 1909',
    area: 'Culebra & surrounding cays',
    phone: '+17877420115',
    websiteUrl: 'https://www.fws.gov/refuge/culebra',
    latitude: 18.3089,
    longitude: -65.2911,
    description:
      'One of the Caribbean’s oldest federal wildlife refuges, protecting important coastal habitat and offshore cays.',
  },

  {
    id: 'sea-turtles',
    sectionId: 'wildlife-land',
    name: 'Sea Turtles',
    image: {
      src: '/images/guide/wildlife/sea-turtles.jpg',
      alt: 'Sea turtle in the waters around Culebra',
    },
    eyebrow: 'Wildlife & Land',
    area: 'Beaches & coastal waters',
    description:
      'Culebra provides important feeding and nesting habitat for protected marine turtles, including green, hawksbill and leatherback turtles.',
  },

  {
    id: 'birding',
    sectionId: 'wildlife-land',
    name: 'Birding',
    image: {
      src: '/images/guide/wildlife/birding.jpg',
      alt: 'Seabirds around Culebra',
    },
    eyebrow: 'Wildlife & Land',
    area: 'Refuge lands & offshore cays',
    description:
      'Protected cays and coastal habitat support resident, migratory and nesting seabirds throughout the archipelago.',
  },

  {
    id: 'hiking-and-trails',
    sectionId: 'wildlife-land',
    name: 'Hiking & Trails',
    image: {
      src: '/images/guide/wildlife/hiking.jpg',
      alt: 'Hiking through the dry landscape of Culebra',
    },
    eyebrow: 'Wildlife & Land',
    area: 'Resaca · Brava · island interior',
    description:
      'Footpaths cross Culebra’s dry landscape and lead to remote beaches, high points and less-traveled corners of the island.',
  },

  {
    id: 'dry-forest',
    sectionId: 'wildlife-land',
    name: 'Dry Forest',
    image: {
      src: '/images/guide/wildlife/dry-forest.jpg',
      alt: 'Subtropical dry forest vegetation in Culebra',
    },
    eyebrow: 'Wildlife & Land',
    area: 'Island interior',
    description:
      'Cacti, scrub and drought-adapted vegetation form a distinctive subtropical dry-forest landscape beyond the shoreline.',
  },

  {
    id: 'conservation',
    sectionId: 'wildlife-land',
    name: 'Conservation',
    image: {
      src: '/images/guide/wildlife/conservation.jpg',
      alt: 'Marine conservation work in Culebra',
    },
    eyebrow: 'Wildlife & Land',
    subtitle: 'Community stewardship',
    area: 'Island-wide',
    websiteUrl: 'https://www.coralations.org',
    description:
      'Local organizations and residents work alongside public agencies to protect reefs, shorelines, wildlife and fragile coastal habitats.',
  },

  // ==========================================
  // 04 COFFEE & CASUAL
  // ==========================================

  {
    id: 'Pandeli',
    sectionId: 'coffee-casual',
    name: 'Pabdeli',
    image: {
      src: '/images/guide/coffee-casual/pandeli.jpg',
      alt: 'Pandeli Culebra',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Breakfast & coffee',
    area: 'Dewey',
    address: '95 Calle Escudero, Culebra, PR 00775',
    phone: '+17877420004',
    latitude: 18.3018,
    longitude: -65.3039,
    description:
      'A small morning café for coffee, pastries and an easy breakfast before heading out across the island.',
  },

  {
    id: 'blac-flamingo-coffee',
    sectionId: 'coffee-casual',
    name: 'Blac Flamingo Coffee',
    image: {
      src: '/images/guide/coffee-casual/blac-flamingo-coffee.jpg',
      alt: 'Blac Flamingo Coffee in Culebra',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Specialty coffee & breakfast',
    area: 'La Romana',
    address: 'PR-250 C-19, Sector La Romana, Culebra, PR 00775',
    phone: '+19393800782',
    latitude: 18.3105,
    longitude: -65.2950,
    description:
      'Specialty coffee and artisan breakfast in an eclectic setting with indoor and outdoor seating.',
  },

  {
    id: 'culebra-coffee',
    sectionId: 'coffee-casual',
    name: 'Culebra Coffee',
    image: {
      src: '/images/guide/coffee-casual/culebra-coffee.jpg',
      alt: 'Culebra Coffee in Dewey',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Puerto Rican coffee & café',
    area: 'Dewey',
    address: '2 Calle Pedro Márquez, Culebra, PR 00775',
    phone: '+13022200773',
    latitude: 18.3017,
    longitude: -65.3024,
    description:
      'A downtown café focused on Puerto Rican coffee, breakfast sandwiches, light fare and smoothies.',
  },

  {
    id: 'latitud-perfecta-cafe',
    sectionId: 'coffee-casual',
    name: 'Latitud Perfecta Café',
    image: {
      src: '/images/guide/coffee-casual/latitud-perfecta-cafe.jpg',
      alt: 'Latitud Perfecta Café in Culebra',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Casual island fare',
    area: 'Dewey',
    address: 'Calle Pedro Márquez, Culebra, PR 00775',
    phone: '+17877420104',
    latitude: 18.3015,
    longitude: -65.3028,
    description:
      'A relaxed town stop for burgers, drinks and unfussy food after a day on the water.',
  },


  {
    id: 'cafe-blue',
    sectionId: 'coffee-casual',
    name: 'Cafe Blue',
    image: {
      src: '/images/guide/coffee-casual/cafe-blue.jpg',
      alt: 'Cafe Blue in Culebra',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Casual island fare',
    area: 'Dewey',
    address: 'Calle Pedro Márquez, Culebra, PR 00775',
    phone: '+17877420104',
    latitude: 18.3015,
    longitude: -65.3028,
    description:
      'A relaxed town stop for burgers, drinks and unfussy food after a day on the water.',
  },


  {
    id: 'kiosk2',
    sectionId: 'coffee-casual',
    name: 'Kiosk #2',
    image: {
      src: '/images/guide/coffee-casual/kiosk2.jpg',
      alt: "Kiosk #2 at Flamenco Beach",
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Beach kiosk',
    area: 'Flamenco Beach',
    address: 'Flamenco Beach, Culebra, PR 00775',
    phone: '+17877420111',
    latitude: 18.3281,
    longitude: -65.3181,
    description:
      'Open-air beach food and cold drinks steps from the sand at Flamenco.',
  },

  // ==========================================
  // 05 DINING
  // ==========================================

  {
    id: 'dinghy-dock',
    sectionId: 'dining',
    name: 'Dinghy Dock',
    image: {
      src: '/images/guide/dining/dinghy-dock.jpg',
      alt: 'Dinghy Dock restaurant on the waterfront in Culebra',
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Waterfront dining',
    area: 'Fulladoza',
    address: '372 Calle Fulladoza, Culebra, PR 00775',
    phone: '+17877420233',
    websiteUrl: 'http://www.dinghydock.com',
    latitude: 18.3006,
    longitude: -65.2994,
    description:
      'A long-standing waterfront gathering place for seafood, drinks and relaxed outdoor dining beside the harbor.',
  },

  {
    id: 'mamacitas',
    sectionId: 'dining',
    name: "Mamacita's",
    image: {
      src: '/images/guide/dining/mamacitas.jpg',
      alt: "Mamacita's restaurant in Culebra",
    },
    eyebrow: 'Eat & Drink',
    subtitle: 'Canal-side dining',
    area: 'Dewey',
    address: '64 Calle Castelar, Culebra, PR 00775',
    phone: '+17877420090',
    websiteUrl: 'https://www.eatatmamacitas.com/',
    latitude: 18.3012,
    longitude: -65.3019,
    description:
      'A canal-side Culebra institution serving Caribbean and Puerto Rican-inspired food, seafood and drinks.',
  },

  // ==========================================
  // 06 GETTING HERE
  // ==========================================

  {
    id: 'culebra-airport',
    sectionId: 'getting-here',
    name: 'Culebra Airport (CPX)',
    image: {
      src: '/images/guide/getting-here/culebra-airport.jpg',
      alt: 'Benjamín Rivera Noriega Airport in Culebra',
    },
    eyebrow: 'Getting Here',
    subtitle: 'Benjamín Rivera Noriega Airport',
    area: 'Culebra',
    address: 'Carretera 251, Culebra, PR 00775',
    latitude: 18.3130,
    longitude: -65.3039,
    description:
      'Culebra’s small airport handles scheduled regional flights, private aviation and charter service.',
  },

  {
    id: 'air-flamenco',
    sectionId: 'getting-here',
    name: 'Air Flamenco',
    image: {
      src: '/images/guide/getting-here/air-flamenco.jpg',
      alt: 'Air Flamenco aircraft serving Culebra',
    },
    eyebrow: 'Getting Here',
    subtitle: 'Regional air service',
    area: 'San Juan · Ceiba · Culebra',
    phone: '+17877241818',
    websiteUrl: 'https://www.booking.flyairflamenco.com/',
    description:
      'Regional commuter service connecting Culebra with airports on Puerto Rico’s main island.',
  },

  {
    id: 'cape-air',
    sectionId: 'getting-here',
    name: 'Cape Air',
    image: {
      src: '/images/guide/getting-here/cape-air.jpg',
      alt: 'Cape Air aircraft serving Culebra',
    },
    eyebrow: 'Getting Here',
    subtitle: 'San Juan air service',
    area: 'San Juan · Culebra',
    websiteUrl: 'https://www.capeair.com/',
    description:
      'Scheduled regional flights connect San Juan with Culebra’s Benjamín Rivera Noriega Airport.',
  },

  {
    id: 'ceiba-ferry-terminal',
    sectionId: 'getting-here',
    name: 'Ceiba Ferry Terminal',
    image: {
      src: '/images/guide/getting-here/ceiba-ferry-terminal.jpg',
      alt: 'Ceiba ferry terminal in Puerto Rico',
    },
    eyebrow: 'Getting Here',
    subtitle: 'Mainland departure point',
    area: 'Ceiba',
    address: 'Roosevelt Roads, Ceiba, PR 00735',
    latitude: 18.2439,
    longitude: -65.6022,
    description:
      'The mainland terminal for passenger ferry service to Culebra, located on the former Roosevelt Roads property.',
  },

  {
    id: 'puerto-rico-ferry',
    sectionId: 'getting-here',
    name: 'Puerto Rico Ferry',
    image: {
      src: '/images/guide/getting-here/puerto-rico-ferry.jpg',
      alt: 'Puerto Rico Ferry service to Culebra',
    },
    eyebrow: 'Getting Here',
    subtitle: 'Ceiba ↔ Culebra',
    area: 'Ceiba · Culebra',
    websiteUrl: 'https://www.puertoricoferry.com/',
    note:
      'Schedules, ticket availability and check-in requirements should always be confirmed before travel.',
    description:
      'Official passenger ferry service connecting Ceiba on Puerto Rico’s main island with Culebra.',
  },

  {
    id: 'san-juan-connection',
    sectionId: 'getting-here',
    name: 'From San Juan',
    image: {
      src: '/images/guide/getting-here/luis-muoz-marn-international-airport.jpg',
      alt: 'Travel from San Juan to Culebra',
    },
    eyebrow: 'Getting Here',
    subtitle: 'Choose air or connect through Ceiba',
    area: 'San Juan → Culebra',
    note:
      'Flying is the most direct option. Ferry travelers first travel overland from San Juan to Ceiba.',
    description:
      'From metropolitan San Juan, visitors can fly directly to Culebra or travel east to Ceiba and continue by ferry.',
  },

  // ==========================================
  // 07 GETTING AROUND
  // ==========================================

  {
    id: 'carlos-jeep-rentals',
    sectionId: 'getting-around',
    name: 'Carlos Jeep Rentals',
    image: {
      src: '/images/guide/getting-around/carlos-jeep-rentals.jpg',
      alt: 'Carlos Jeep Rentals in Culebra',
    },
    eyebrow: 'Getting Around',
    subtitle: 'Carlos Jeep Rentals',
    area: 'Near the airport',
    address: 'PR-250, Culebra, PR 00775',
    phone: '+17877423514',
    websiteUrl: 'https://www.carlosjeeprental.com',
    latitude: 18.3119,
    longitude: -65.3031,
    description:
      'Jeeps are a popular choice for Culebra’s hills, beach access roads and relaxed open-air island travel.',
  },

  {
    id: 'jerry-jeep-rentals',
    sectionId: 'getting-around',
    name: 'Jerrys Jeep Rentals',
    image: {
      src: '/images/guide/getting-around/jerrys-jeep-rentals.jpg',
      alt: 'Jerrys Jeeps & Golf carts rental in Culebra',
    },
    eyebrow: 'Getting Around',
    subtitle: "Jerry's Jeeps & Carts",
    area: 'Near the airport',
    address: 'PR-250, Culebra, PR 00775',
    phone: '+17877420587',
    latitude: 18.3122,
    longitude: -65.3039,
    description:
      'Open-air carts provide a casual way to move between town, beaches and accommodations around the island.',
  },

  {
    id: 'chocosbroncorental',
    sectionId: 'getting-around',
    name: 'Chocos Bronco Rentals',
    image: {
      src: '/images/guide/getting-around/chocos-bronco-rentals.jpg',
      alt: 'Chocos Bronco rental in Culebra',
    },
    eyebrow: 'Getting Around',
    subtitle: "Chocos Bronco Rental",
    area: 'Culebra',
    address: 'PR-250, Culebra, PR 00775',
    phone: '+17877423537',
    latitude: 18.3121,
    longitude: -65.3028,
    description:
      'Chocos Bronco Rentals for visitors who prefer an Broncos.',
  },

  {
    id: 'island-taxi',
    sectionId: 'getting-around',
    name: 'Taxis & Públicos',
    image: {
      src: '/images/guide/getting-around/island-taxis.jpg',
      alt: 'Island Taxis in Culebra',
    },
    eyebrow: 'Getting Around',
    area: 'Ferry dock · Airport · Flamenco',
    description:
      'Local taxi vans and drivers connect the ferry dock, airport, Flamenco Beach and other common island destinations.',
  },

  // ==========================================
  // 08 ISLAND ESSENTIALS
  // ==========================================

  {
    id: 'supermercado-milka',
    sectionId: 'essentials',
    name: 'Supermercado Milka',
    image: {
      src: '/images/guide/essentials/supermercado-milka.jpg',
      alt: 'Grocery shopping in Culebra',
    },
    eyebrow: 'Essentials',
    subtitle: 'Econo',
    area: 'Dewey',
    address: 'Calle William Font, Culebra, PR 00775',
    phone: '+17877420528',
    latitude: 18.3022,
    longitude: -65.3025,
    description:
      'Full-service grocery shopping for food, household supplies and the basics of everyday island life.',
  },
{
    id: 'supermercado-costa-del-sol',
    sectionId: 'essentials',
    name: 'Supermercado Costa Del Sol',
    image: {
      src: '/images/guide/essentials/supermercado-costa-del-sol.jpg',
      alt: 'Grocery shopping in Culebra',
    },
    eyebrow: 'Essentials',
    subtitle: 'Econo',
    area: 'Dewey',
    address: 'Calle William Font, Culebra, PR 00775',
    phone: '+17877420528',
    latitude: 18.3022,
    longitude: -65.3025,
    description:
      'Full-service grocery shopping for food, household supplies and the basics of everyday island life.',
  },

  {
    id: 'supermercado-mayras',
    sectionId: 'essentials',
    name: 'Supermercado Mayras',
    image: {
      src: '/images/guide/essentials/supermercado-mayras.jpg',
      alt: 'Supermercado Mayras in Culebra',
    },
    eyebrow: 'Essentials',
    subtitle: 'Farmacia Culebra',
    area: 'Dewey',
    address: 'Calle Pedro Márquez, Culebra, PR 00775',
    phone: '+17877420710',
    latitude: 18.3015,
    longitude: -65.3027,
    description:
      ' A local grocery and pharmacy for everyday needs, including food, household supplies and personal care items',
  },

  {
    id: 'emergency-medical-services',
    sectionId: 'essentials',
    name: 'Emergency And Medical Services',
    image: {
      src: '/images/guide/essentials/emergency-medical.jpg',
      alt: 'Emergency and Medical services in Culebra',
    },

    eyebrow: 'Essentials',
    subtitle: 'Culebra health services',
    area: 'Culebra',
    address: 'Calle Unión, Culebra, PR 00775',
    phone: '+17877420110',
    latitude: 18.3061,
    longitude: -65.3014,
    description:
      'Local emergency and medical services provide basic care and urgent evaluation, with more advanced treatment available on Puerto Rico’s main island.',
  },

  {
    id: 'atm-banking',
    sectionId: 'essentials',
    name: 'ATM / Banking',
    image: {
      src: '/images/guide/essentials/atm-banking.jpg',
      alt: 'ATM and banking services in Culebra',
    },
    eyebrow: 'Essentials',
    subtitle: 'Cash access',
    area: 'Dewey',
    description:
      'Cash remains useful around the island, particularly for some transportation, kiosks and smaller local businesses.',
  },

  {
    id: 'ferreteria-gonzalez',
    sectionId: 'essentials',
    name: 'Ferreteria Gonzalez',
    image: {
      src: '/images/guide/essentials/ferreteria gonzalez.jpg',
      alt: 'Ferreteria Gonzalez Hardware Store in Culebra',
    },
    eyebrow: 'Essentials',
    area: 'Culebra',
    description:
      'Ferreteria Gonzalez Island hardware and household suppliers are useful for tools, repairs, marine needs and the realities of maintaining a home here.',
  },

  {
    id: 'post-office',
    sectionId: 'essentials',
    name: 'Post Office',
    image: {
      src: '/images/guide/essentials/post-office.jpg',
      alt: 'United States Post Office in Culebra',
    },
    eyebrow: 'Essentials',
    subtitle: 'USPS Culebra',
    area: 'Dewey',
    address: 'Culebra, PR 00775',
    websiteUrl: 'https://www.usps.com/',
    description:
      'USPS service supports island mail, packages and post-office boxes as part of everyday life in Culebra.',
  },

  // ==========================================
  // 09 ISLAND STORY
  // ==========================================

  {
    id: 'early-inhabitants',
    sectionId: 'island-story',
    name: 'Early Inhabitants',
    image: {
      src: '/images/guide/island-story/early-inhabitants.jpg',
      alt: 'Indigenous history of Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'Before European colonization',
    area: 'Culebra',
    note: 'Era: Pre-colonial',
    description:
      'Indigenous peoples Taino Arawak lived throughout the northeastern Caribbean long before European colonization, using these islands and waters for settlement, fishing and navigation.',
  },

  {
    id: 'spanish-period',
    sectionId: 'island-story',
    name: 'Spanish Period',
    image: {
      src: '/images/guide/island-story/spainish-period.jpg',
      alt: 'Spanish-era history of Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'Colonial era',
    area: 'Culebra',
    note: 'Era: Spanish rule',
    description:
      'Culebra remained within the Spanish Caribbean world for centuries before formal settlement accelerated in the nineteenth century.',
  },

  {
    id: 'island-life',
    sectionId: 'island-story',
    name: 'Pirate Life',
    image: {
      src: '/images/guide/island-story/pirates.jpg',
      alt: 'Historic community life in Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'An island community takes shape',
    area: 'Dewey',
    note: 'Era: Late 19th and early 20th centuries',
    description:
      'Fishing, farming, maritime activity and close community ties shaped life as permanent settlement developed.',
  },

  {
    id: 'us-navy-presence',
    sectionId: 'island-story',
    name: 'U.S. Navy Presence',
    image: {
      src: '/images/guide/island-story/us-navy.jpg',
      alt: 'Historic United States Navy presence in Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'Military era',
    area: 'Culebra',
    note: 'Era: 20th century',
    description:
      'Large parts of Culebra and its surrounding waters became associated with U.S. military training, profoundly affecting the island and its residents.',
  },

  {
    id: 'community-resistance',
    sectionId: 'island-story',
    name: 'Community Resistance',
    image: {
      src: '/images/guide/island-story/resistance.jpg',
      alt: 'Community activism in Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'Island activism',
    area: 'Culebra',
    note: 'Era: 1970s',
    description:
      'Residents and supporters organized sustained civil action against military exercises, making the struggle for Culebra nationally significant.',
  },

  {
    id: 'navy-departure',
    sectionId: 'island-story',
    name: 'A New Chapter',
    image: {
      src: '/images/guide/island-story/culebra-1975.jpg',
      alt: 'Culebra after the end of United States Navy exercises',
    },
    eyebrow: 'Island Story',
    subtitle: 'Transition',
    area: 'Culebra',
    note: 'Era: 1975',
    description:
      'The end of Navy exercises on Culebra in 1975 marked a defining transition for the island and its relationship with its protected landscape.',
  },

  {
    id: 'conservation-legacy',
    sectionId: 'island-story',
    name: 'Conservation Legacy',
    image: {
      src: '/images/guide/island-story/conservation2.jpg',
      alt: 'Protected landscape and wildlife habitat in Culebra',
    },
    eyebrow: 'Island Story',
    subtitle: 'Protection & stewardship',
    area: 'Culebra',
    note: 'Era: Continuing legacy',
    description:
      'Refuge lands, marine protection and community stewardship became central to preserving the landscapes that define modern Culebra.',
  },

  {
    id: 'culebra-today',
    sectionId: 'island-story',
    name: 'Culebra Today',
    image: {
      src: '/images/guide/island-story/culebra-today.jpg',
      alt: 'Present-day Culebra landscape',
    },
    eyebrow: 'Island Story',
    subtitle: 'An island intentionally apart',
    area: 'Culebra',
    note: 'Era: Present',
    description:
      'Culebra remains small, lightly developed and deeply shaped by the tension between access, community, conservation and change.',
  },
];
export default function GuidePage() {
  return (
    <>
      <SiteHeader />

      <main>
        <PageHero
          eyebrow="Island Guide"
          title="A slower rhythm, kept intentionally intact."
          intro="No traffic lights. No high-rises. Fishing boats at dawn, reef-clear water by noon, and evenings measured in shades of gold."
          image="/images/culture.png"
          imageAlt="The white sand crescent and turquoise water of Flamenco Beach, Culebra"
        />

        {/* Guide navigation */}
        <section className="border-b border-border px-6 md:px-12">
          <div className="mx-auto max-w-[1600px] overflow-x-auto">
            <nav className="flex min-w-max gap-8 py-6 md:gap-10">
              {GUIDE_SECTIONS.map((section) => (
                <a
                  key={section.id}
                  href={`#${section.id}`}
                  className="text-[11px] font-light uppercase tracking-[0.22em] text-muted-foreground transition-colors hover:text-foreground"
                >
                  {section.title}
                </a>
              ))}
            </nav>
          </div>
        </section>

        {/* Guide sections */}
        <section className="px-6 py-20 md:px-12 md:py-28">
          <div className="mx-auto max-w-[1600px]">
            <div className="space-y-24 md:space-y-32">
              {GUIDE_SECTIONS.map((section, i) => (
                <section
                  key={section.id}
                  id={section.id}
                  className="scroll-mt-24"
                >
                  <Reveal delay={i * 60}>
                    <div className="grid gap-10 md:grid-cols-12 md:gap-12">
                      <div className="md:col-span-3">
                        <p className="text-xs font-light uppercase tracking-[0.28em] text-accent">
                          {section.number} / {section.title}
                        </p>

                        <h2 className="mt-5 font-serif text-2xl font-light leading-[1.2] text-foreground md:text-3xl">
                          {section.headline}
                        </h2>

                        <p className="mt-5 max-w-sm text-sm font-light leading-relaxed text-muted-foreground">
                          {section.description}
                        </p>
                      </div>

                      <div className="min-w-0 md:col-span-9">
                        <div className="flex gap-5 overflow-x-auto pb-4">
                          {culebraGuideItems
                            .filter((item) => item.sectionId === section.id)
                            .map((item) => (
                              <article
                                key={item.id}
                                className="w-[78vw] max-w-[280px] shrink-0 sm:w-[240px] lg:w-[220px]"
                              >
                                <div className="aspect-[4/3] overflow-hidden bg-muted">
                                  {item.image?.src ? (
                                    <img
                                      src={item.image.src}
                                      alt={item.image.alt}
                                      className="h-full w-full object-cover"
                                    />
                                  ) : (
                                    <div className="flex h-full items-center justify-center">
                                      <span className="text-[10px] font-light uppercase tracking-[0.2em] text-muted-foreground">
                                        Image
                                      </span>
                                    </div>
                                  )}
                                </div>

                                <div className="pt-5">
                                  <p className="text-[10px] font-light uppercase tracking-[0.22em] text-accent">
                                    {item.subtitle || item.area || item.eyebrow}
                                  </p>

                                  <h3 className="mt-2 font-serif text-xl font-light leading-tight text-foreground">
                                    {item.name}
                                  </h3>

                                  <p className="mt-3 text-sm font-light leading-relaxed text-muted-foreground">
                                    {item.description}
                                  </p>

                                  {item.address && (
                                    <p className="mt-4 text-xs font-light leading-relaxed text-muted-foreground">
                                      {item.address}
                                    </p>
                                  )}

                                  {item.phone && (
                                    <a
                                      href={`tel:${item.phone}`}
                                      className="mt-2 block text-xs font-light text-foreground"
                                    >
                                      {item.phone}
                                    </a>
                                  )}

                                  {item.websiteUrl && (
                                    <a
                                      href={item.websiteUrl}
                                      target="_blank"
                                      rel="noreferrer"
                                      className="mt-3 inline-block text-[10px] font-light uppercase tracking-[0.2em] text-foreground"
                                    >
                                      Visit website →
                                    </a>
                                  )}
                                </div>
                              </article>
                            ))}
                        </div>
                      </div>
                    </div>
                  </Reveal>
                </section>
              ))}
            </div>
          </div>
        </section>

        {/* CTA */}
        <section className="bg-primary px-6 py-24 text-primary-foreground md:px-12 md:py-32">
          <div className="mx-auto flex max-w-[1600px] flex-col items-start gap-8">
            <Reveal>
              <h2 className="max-w-3xl text-balance font-serif text-3xl font-light leading-[1.1] md:text-4xl">
                When you are ready to find your place here.
              </h2>
            </Reveal>

            <Reveal delay={120}>
              <Link
                href="/buyers"
                className="group inline-flex items-center gap-3 text-xs font-light uppercase tracking-[0.24em]"
              >
                Explore buying on Culebra
                <span className="inline-block h-px w-10 bg-primary-foreground transition-all duration-500 group-hover:w-16" />
              </Link>
            </Reveal>
          </div>
        </section>
      </main>

      <SiteFooter />
    </>
  )
}