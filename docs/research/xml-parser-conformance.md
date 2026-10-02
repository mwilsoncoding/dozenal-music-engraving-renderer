# XML parser conformance

## Product boundary

The renderer owns the parser and MusicXML score model; it may not use third-party
Rust crates. This report covers only an uncompressed `score-partwise` document.
“Full conformance” below means conformance to the selected XML 1.0 processor
requirements and Namespaces in XML requirements, not merely accepting common
MusicXML files. It does not by itself promise MusicXML DTD or XSD validity.

## What the Recommendations require

- An XML document is well-formed when it satisfies the XML grammar and all
  well-formedness constraints; a conforming XML processor must make an error
  report if it detects a violation. A document is valid only if it has an
  associated DTD and satisfies all validity constraints. [XML 1.0 §2.1](https://www.w3.org/TR/xml/#sec-well-formed),
  [§2.8](https://www.w3.org/TR/xml/#sec-prolog-dtd), [§5.1](https://www.w3.org/TR/xml/#sec-validity).
- XML defines two processor conformance classes. Both must check well-formedness
  and report detected well-formedness violations. A non-validating processor
  must process the internal subset, but need not read the external subset or
  external parameter entities. A validating processor must read and process
  declarations needed for validation, including the external subset and
  external parameter entities when present, and must report validity errors.
  Thus “fully conforming” still requires choosing a class; it does not mean
  every processor validates. [XML 1.0 §5](https://www.w3.org/TR/xml/#sec-conformance),
  [§5.1](https://www.w3.org/TR/xml/#sec-validity),
  [§4.4](https://www.w3.org/TR/xml/#sec-include-if-valid).
- The XML declaration, when present, must be at the start and its `version`
  and `encoding` pseudo-attributes follow the declaration grammar. A conforming
  processor must support UTF-8 and UTF-16 and follow XML's encoding detection
  and declaration consistency rules; it need not support every other encoding.
  XML 1.0 also specifies byte-order-mark handling and end-of-line normalization.
  [XML 1.0 §2.8](https://www.w3.org/TR/xml/#sec-prolog-dtd),
  [§2.11](https://www.w3.org/TR/xml/#sec-line-ends),
  [§4.3.3](https://www.w3.org/TR/xml/#charencoding),
  [§4.3.4](https://www.w3.org/TR/xml/#sec-guessing).
- Names must obey the XML `Name` production; XML 1.0 also defines the allowed
  character repertoire and name-start/name-character rules. Namespace-aware
  processing adds namespace constraints: prefixes are bound by namespace
  declarations, `xml` has its reserved binding, `xmlns` is reserved, and an
  expanded name is the namespace name plus local name (or no namespace).
  Namespace names are compared as URI references/strings, not by resolving or
  dereferencing them. [XML 1.0 §2.3](https://www.w3.org/TR/xml/#sec-common-syn),
  [Namespaces 1.0 §2](https://www.w3.org/TR/xml-names/#ns-decl),
  [§3](https://www.w3.org/TR/xml-names/#ns-using), [§6.2](https://www.w3.org/TR/xml-names/#defaulting).
- Namespace well-formedness includes rejecting undeclared prefixes and
  namespace constraint violations. Namespace-aware consumers identify names by
  namespace name and local name; a default namespace applies to unprefixed
  element names, not unprefixed attributes. [Namespaces 1.0
  §3](https://www.w3.org/TR/xml-names/#ns-using),
  [§6.2](https://www.w3.org/TR/xml-names/#defaulting).
- General and parameter entities, replacement text, character references,
  internal/external subsets, and external parsed entities have specified
  declaration and expansion rules. External entities can cause resource access;
  the XML Recommendation does not define a network sandbox, maximum expansion,
  timeout, or safe-resource policy. Refusing external resources is a product
  restriction, not a general XML processor requirement; ignoring declarations
  that the selected processor class must process would not be conforming.
  [XML 1.0 §4](https://www.w3.org/TR/xml/#sec-physical-struct),
  [§4.2](https://www.w3.org/TR/xml/#sec-entities), [§4.3.2](https://www.w3.org/TR/xml/#sec-external-ent),
  [§4.4](https://www.w3.org/TR/xml/#sec-include-if-valid).

## Source positions and diagnostics

XML 1.0 and Namespaces in XML specify syntax, processing, and error obligations,
not a Rust API, event model, diagnostic wording, byte/character offset scheme,
or line/column accuracy contract. They do not require source positions to be
retained or exposed. Any guarantee of byte offsets, Unicode scalar positions,
line/column, or spans is therefore a renderer decision. Define positions
relative to original input bytes or decoded/normalized text, and document how
encoding and newline normalization affect them. [XML 1.0
§2.11](https://www.w3.org/TR/xml/#sec-line-ends),
[§5.2](https://www.w3.org/TR/xml/#sec-xml-proc).

## Defensible baseline contract

One defensible product profile is a namespace-aware, non-validating XML 1.0
processor. It checks well-formedness, processes the internal subset, supports
UTF-8/UTF-16 and XML normalization/reference rules, then accepts only the
`score-partwise` root in a chosen MusicXML namespace. It does not claim DTD or
XSD validity. External-resource access and resource limits are product
decisions: if the profile refuses external declarations/entities or inputs
exceeding finite limits, it must reject/report them rather than silently ignore
declarations or claim acceptance as an unrestricted XML processor. Keep the
XML-processing layer distinct from the later MusicXML root/model allowlist.
This is a suggested contract, not a W3C requirement.

## Human decisions still required

- validating versus non-validating processing;
- whether to accept a `DOCTYPE`, internal subset, external subset, and general
  or parameter entities, and the resolver/cache/URI and expansion policy;
- the exact accepted root namespace URI, treatment of no namespace, and
  prefix-independent matching;
- supported encodings beyond mandatory UTF-8/UTF-16; explicitly decide to
  accept XML 1.0 only and reject XML 1.1, or define support for another version;
- byte-order-mark, malformed encoding, unsupported encoding, and XML
  declaration mismatch diagnostics;
- diagnostic schema, source-position units, normalization behavior, and
  recovery versus fail-fast parsing;
- limits and security policy for input size, depth, entities, external-resource
  access, time, and memory;
- whether MusicXML DTD/XSD validity is promised, and if so which schema/version
  and whether it is a separate validation phase.

## Sources consulted

- [W3C XML 1.0 (Fifth Edition)](https://www.w3.org/TR/xml/)
- [W3C Namespaces in XML 1.0 (Third Edition)](https://www.w3.org/TR/xml-names/)
