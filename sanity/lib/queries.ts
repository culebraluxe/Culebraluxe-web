import {defineQuery} from 'next-sanity'

export const PROPERTY_BY_SLUG_QUERY = defineQuery(`
  *[
    _type == "property" &&
    slug.current == $slug
  ][0] {
    _id,
    title,
    slug,
    listingId,
    standardStatus,
    propertyType,
    listPrice,
    featured,

    // location
    streetNumber,
    streetName,
    city,
    stateOrProvince,
    neighborhood,
    latitude,
    longitude,

    // details
    bedroomsTotal,
    bathroomsFull,
    bathroomsHalf,
    bathroomsTotal,
    livingArea,
    lotSizeArea,
    lotSizeUnits,
    yearBuilt,
    stories,
    parkingSpaces,
    viewType,
    waterAccess,
    beachAccess,
    amenities,

    // media
    heroImage,
    "gallery": gallery[]{
      ...,
      "alt": coalesce(alt, ^.title),
      caption
    },
    videoUrl,
    virtualTourUrl,

    // editorial
    heroTitle,
    tagline,
    shortDescription,
    editorialDescription,
    architecture,
    lifestyleTags,

    // agent / office
    listingAgentName,
    listingAgentEmail,
    listingAgentPhone,
    listingOffice
  }
`)

// Homepage "Find Your Place in Culebra" links section.
// Returns properties with featured ones first, then by price, capped at 8,
// so the section always renders cards even if none are flagged featured yet.
export const HOME_PROPERTIES_QUERY = defineQuery(`
  *[_type == "property" && defined(slug.current)]
    | order(featured desc, listPrice desc)[0...8] {
    _id,
    title,
    slug,
    listPrice,
    featured,
    propertyType,
    neighborhood,
    bedroomsTotal,
    bathroomsTotal,
    livingArea,
    lotSizeArea,
    lotSizeUnits,
    viewType,
    heroImage
  }
`)
