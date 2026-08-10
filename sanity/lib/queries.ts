import {defineQuery} from 'next-sanity'

export const PROPERTY_BY_SLUG_QUERY = defineQuery(`
  *[
    _type == "property" &&
    slug.current == $slug
  ][0] {
    _id,
    title,
    slug,
    listPrice,
    standardStatus,
    propertyType,
    bedroomsTotal,
    bathroomsTotal,
    livingArea,
    lotSizeArea,
    lotSizeUnits,
    yearBuilt,
    neighborhood,
    viewType,
    heroImage,
    gallery,
    shortDescription,
    editorialDescription,
    amenities,
    latitude,
    longitude
  }
`)
