# Parallel delivery review: chapters and reusable assets

Date: 2026-10-23
Status: Plan review; implementation and verification remain separate.

## Product and scope

The product vision in README.md favors structured story information, stable
reusable assets and preserved image alternatives. The selected increments are
[F02 chapters](chapter-management.md) and
[F04 with structured F06 associations](asset-library.md). Their plans follow the
approved milestone requirements and existing revisioned project aggregate.

F02 needs variable panel counts and a chapter-owned composition page to deliver
its five-panel deletion workflow. This increment also supplies the expression
field needed by the association workflow. It does not complete F03 panel
planning or F19 multi-page composition. F06 reference-image access depends on
F05, and backup acceptance remains dependent on F07.

## Dependency and ownership review

```text
Existing F01 project persistence
  + shared document version and lossless legacy-read migration
      + F02 chapter/panel ownership and variable panel counts
      + F04 project asset library + F06 structured associations
          -> combined save/reopen, image-retention and browser verification

Later: F05 references -> complete F06 reference access
       F03/F19 full planning/pages -> F07 complete backup -> milestone verification
```

The two implementation streams can proceed in parallel after agreeing the shared
model hooks. Chapter work owns the workspace shape, chapter lifecycle, server
image validation and primary editor integration. Asset work owns separate model
and browser modules and its CSS. Root serializes shared-field/component wiring,
shared documentation and combined verification. Keeping project-wide flat panel
IDs avoids restructuring the durable upload catalog and asset usage lookup.

## Required safeguards

- Preserve every existing panel/image identity, URL, alternative and crop setting
  during legacy decoding. Missing chapters mean old data; an explicitly empty
  chapter list must stay empty.
- Reject old or unsupported document versions on mutation. Deserializing legacy
  storage must not erase the original version marker of incoming old-client
  saves. Merely documenting mixed-version writes as unsupported is insufficient:
  omitted fields could otherwise erase chapters and assets.
- Compare image records by stable panel ID, replacing the current positional zip.
  New panels cannot introduce image records; retained panels cannot replace them
  through ordinary edits. Deleted panels may be omitted.
- Migrate the upload catalog's four-panel SQL constraint. Keep panel IDs positive
  and within the signed database integer range, with a persisted allocation
  counter that prevents identity reuse after deletion.
- Validate exactly one chapter owner per panel, chapter-local placements and no
  repeated placement across pages. Validate all asset references against the same
  project's library and expected asset kind.
- Preserve the library when deleting chapters and panels. Block deletion of an
  associated asset and identify its referring panels. Disable destructive
  controls during pending or uncertain uploads.
- Adapt existing browser checks that assume root-level layout/slots. Test combined
  chapter deletion, shared definitions and image alternatives, including project
  isolation, revision conflicts, old-client rejection and reload retention.

## Review outcome

The proposed architecture and parallel boundaries are suitable for this scoped
delivery. The chapter draft incorporates the migration, image-identity and upload
safeguards. The asset draft must replace its mixed-version caveat with the shared
mandatory version guard and keep deferred reference viewing explicit. Implementation
can follow orchestration review under the user's request to plan and implement;
this review does not itself assert feature completion or successful verification.
