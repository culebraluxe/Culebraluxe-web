// Generic workflow XML authoring pipeline.
//
//   XML source
//     -> mini-xml (bounded, dependency-free XML parser)
//     -> xml-parser (grammar -> ParsedProcessDefinition / ProcessGraph)
//     -> graph-validator (generic validation on ProcessGraph)
//     -> deployment (upsertProcessDefinition in workflow_app/definitions/deploy.ts)
//
// None of these modules import CulebraLuxe domain code. The engine remains
// domain-neutral and is never modified to understand XML.

export {
  parseXml,
  XmlParseError,
  isXmlElement,
  type XmlElement,
  type XmlText,
  type XmlNode,
} from './mini-xml'

export {
  parseProcessDefinitionXml,
  XmlGrammarError,
  type ParsedProcessDefinition,
} from './xml-parser'

export {
  validateProcessGraph,
  type GraphValidationResult,
} from './graph-validator'
