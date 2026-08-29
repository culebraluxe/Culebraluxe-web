# Decision Studio

Internal three-path decision analysis at `/portal/decision-analysis`.

Live inputs, on-screen tree, client-side PDF via `pdf-lib`. No new dependencies.

Portal middleware already gates `/portal/:path*`, so this stays off the public marketing site.

Same fold-back math as the spreadsheet model: sale minus selling costs minus future capex plus salvage minus placeholder tax, then present-valued at the discount rate.

Option 3 is two chance nodes: P(stabilize) times conditional buyer prices, plus a fail salvage branch.

Do not treat the ranking as an appraisal. The appraisal input is still the number that matters most.
