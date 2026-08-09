import {defineField, defineType} from 'sanity'

export const property = defineType({
  name: 'property',
  title: 'Property',
  type: 'document',

  groups: [
    {name: 'listing', title: 'Listing'},
    {name: 'location', title: 'Location'},
    {name: 'details', title: 'Property Details'},
    {name: 'media', title: 'Media'},
    {name: 'editorial', title: 'CulebraLuxe Editorial'},
    {name: 'source', title: 'Source / IDX'},
    {name: 'seo', title: 'SEO'},
  ],

  fields: [
    // ------------------------------------------------
    // LISTING / IDENTITY
    // ------------------------------------------------

    defineField({
      name: 'title',
      title: 'Property Name',
      type: 'string',
      group: 'listing',
      validation: (rule) => rule.required(),
    }),

    defineField({
      name: 'slug',
      title: 'URL Slug',
      type: 'slug',
      group: 'listing',
      options: {source: 'title'},
      validation: (rule) => rule.required(),
    }),

    defineField({
      name: 'listingId',
      title: 'Listing ID',
      type: 'string',
      group: 'listing',
    }),

    defineField({
      name: 'standardStatus',
      title: 'Status',
      type: 'string',
      group: 'listing',
      options: {
        list: [
          {title: 'Active', value: 'Active'},
          {title: 'Coming Soon', value: 'ComingSoon'},
          {title: 'Pending', value: 'Pending'},
          {title: 'Closed / Sold', value: 'Closed'},
          {title: 'Private / Off Market', value: 'Private'},
        ],
      },
    }),

    defineField({
      name: 'propertyType',
      title: 'Property Type',
      type: 'string',
      group: 'listing',
      options: {
        list: [
          'Single Family',
          'Condominium',
          'Villa',
          'Land',
          'Multi Family',
          'Commercial',
          'Other',
        ],
      },
    }),

    defineField({
      name: 'listPrice',
      title: 'List Price',
      type: 'number',
      group: 'listing',
    }),

    defineField({
      name: 'originalListPrice',
      title: 'Original List Price',
      type: 'number',
      group: 'listing',
    }),

    defineField({
      name: 'featured',
      title: 'Featured Property',
      type: 'boolean',
      group: 'listing',
      initialValue: false,
    }),

    // ------------------------------------------------
    // LOCATION
    // ------------------------------------------------

    defineField({
      name: 'streetNumber',
      title: 'Street Number',
      type: 'string',
      group: 'location',
    }),

    defineField({
      name: 'streetName',
      title: 'Street Name',
      type: 'string',
      group: 'location',
    }),

    defineField({
      name: 'unitNumber',
      title: 'Unit Number',
      type: 'string',
      group: 'location',
    }),

    defineField({
      name: 'city',
      title: 'City',
      type: 'string',
      group: 'location',
      initialValue: 'Culebra',
    }),

    defineField({
      name: 'stateOrProvince',
      title: 'State / Province',
      type: 'string',
      group: 'location',
      initialValue: 'PR',
    }),

    defineField({
      name: 'postalCode',
      title: 'Postal Code',
      type: 'string',
      group: 'location',
    }),

    defineField({
      name: 'neighborhood',
      title: 'Neighborhood / Area',
      type: 'string',
      group: 'location',
    }),

    defineField({
      name: 'latitude',
      title: 'Latitude',
      type: 'number',
      group: 'location',
    }),

    defineField({
      name: 'longitude',
      title: 'Longitude',
      type: 'number',
      group: 'location',
    }),

    // ------------------------------------------------
    // PROPERTY DETAILS
    // ------------------------------------------------

    defineField({
      name: 'bedroomsTotal',
      title: 'Bedrooms',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'bathroomsFull',
      title: 'Full Bathrooms',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'bathroomsHalf',
      title: 'Half Bathrooms',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'bathroomsTotal',
      title: 'Total Bathrooms',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'livingArea',
      title: 'Living Area',
      type: 'number',
      group: 'details',
      description: 'Interior living area in square feet',
    }),

    defineField({
      name: 'lotSizeArea',
      title: 'Lot Size',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'lotSizeUnits',
      title: 'Lot Size Units',
      type: 'string',
      group: 'details',
      options: {
        list: ['Square Feet', 'Acres'],
      },
    }),

    defineField({
      name: 'yearBuilt',
      title: 'Year Built',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'stories',
      title: 'Stories',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'parkingSpaces',
      title: 'Parking Spaces',
      type: 'number',
      group: 'details',
    }),

    defineField({
      name: 'viewType',
      title: 'View',
      type: 'array',
      group: 'details',
      of: [{type: 'string'}],
      options: {
        list: [
          'Ocean',
          'Bay',
          'Beach',
          'Harbor',
          'Island',
          'Mountain',
          'Sunrise',
          'Sunset',
        ],
      },
    }),

    defineField({
      name: 'waterAccess',
      title: 'Water Access',
      type: 'boolean',
      group: 'details',
    }),

    defineField({
      name: 'beachAccess',
      title: 'Beach Access',
      type: 'boolean',
      group: 'details',
    }),

    defineField({
      name: 'amenities',
      title: 'Amenities',
      type: 'array',
      group: 'details',
      of: [{type: 'string'}],
    }),

    // ------------------------------------------------
    // MEDIA
    // ------------------------------------------------

    defineField({
      name: 'heroImage',
      title: 'Hero Image',
      type: 'image',
      group: 'media',
      options: {
        hotspot: true,
      },
    }),

    defineField({
      name: 'gallery',
      title: 'Gallery',
      type: 'array',
      group: 'media',
      of: [
        {
          type: 'image',
          options: {
            hotspot: true,
          },
          fields: [
            {
              name: 'caption',
              title: 'Caption',
              type: 'string',
            },
            {
              name: 'alt',
              title: 'Alt Text',
              type: 'string',
            },
          ],
        },
      ],
    }),

    defineField({
      name: 'videoUrl',
      title: 'Video URL',
      type: 'url',
      group: 'media',
    }),

    defineField({
      name: 'virtualTourUrl',
      title: 'Virtual Tour URL',
      type: 'url',
      group: 'media',
    }),

    // ------------------------------------------------
    // CULEBRALUXE EDITORIAL
    // ------------------------------------------------

    defineField({
      name: 'heroTitle',
      title: 'Hero Title',
      type: 'string',
      group: 'editorial',
    }),

    defineField({
      name: 'tagline',
      title: 'Tagline',
      type: 'string',
      group: 'editorial',
    }),

    defineField({
      name: 'shortDescription',
      title: 'Short Description',
      type: 'text',
      rows: 3,
      group: 'editorial',
    }),

    defineField({
      name: 'editorialDescription',
      title: 'CulebraLuxe Description',
      type: 'array',
      group: 'editorial',
      of: [{type: 'block'}],
    }),

    defineField({
      name: 'publicRemarks',
      title: 'MLS / Public Remarks',
      type: 'text',
      rows: 6,
      group: 'editorial',
      description:
        'Raw listing remarks. Keep separate from CulebraLuxe editorial copy.',
    }),

    defineField({
      name: 'architecture',
      title: 'Architecture / Design',
      type: 'text',
      rows: 4,
      group: 'editorial',
    }),

    defineField({
      name: 'lifestyleTags',
      title: 'Lifestyle Tags',
      type: 'array',
      group: 'editorial',
      of: [{type: 'string'}],
    }),

    // ------------------------------------------------
    // AGENT / OFFICE
    // ------------------------------------------------

    defineField({
      name: 'listingAgentName',
      title: 'Listing Agent Name',
      type: 'string',
      group: 'listing',
    }),

    defineField({
      name: 'listingAgentEmail',
      title: 'Listing Agent Email',
      type: 'string',
      group: 'listing',
    }),

    defineField({
      name: 'listingAgentPhone',
      title: 'Listing Agent Phone',
      type: 'string',
      group: 'listing',
    }),

    defineField({
      name: 'listingOffice',
      title: 'Listing Office',
      type: 'string',
      group: 'listing',
    }),

    // ------------------------------------------------
    // SOURCE / FUTURE IDX
    // ------------------------------------------------

    defineField({
      name: 'sourceType',
      title: 'Source Type',
      type: 'string',
      group: 'source',
      initialValue: 'manual',
      options: {
        list: [
          {title: 'Manual', value: 'manual'},
          {title: 'IDX / MLS', value: 'idx'},
          {title: 'Private', value: 'private'},
        ],
      },
    }),

    defineField({
      name: 'sourceProvider',
      title: 'Source Provider',
      type: 'string',
      group: 'source',
    }),

    defineField({
      name: 'sourceListingKey',
      title: 'Source Listing Key',
      type: 'string',
      group: 'source',
    }),

    defineField({
      name: 'sourceModifiedAt',
      title: 'Source Modified At',
      type: 'datetime',
      group: 'source',
    }),

    defineField({
      name: 'lastSyncedAt',
      title: 'Last Synced At',
      type: 'datetime',
      group: 'source',
    }),

    // ------------------------------------------------
    // SEO
    // ------------------------------------------------

    defineField({
      name: 'seoTitle',
      title: 'SEO Title',
      type: 'string',
      group: 'seo',
    }),

    defineField({
      name: 'seoDescription',
      title: 'SEO Description',
      type: 'text',
      rows: 3,
      group: 'seo',
    }),
  ],

  preview: {
    select: {
      title: 'title',
      subtitle: 'standardStatus',
      media: 'heroImage',
    },
  },
})
