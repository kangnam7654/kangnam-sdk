# ziwei-engine

Pure Rust Zi Wei Dou Shu (자미두수) chart engine.

## Calculation Profile

The current product profile is locked as
`iztro_compatible_kr_service` / `v1`:

- **Compatibility target**: `iztro-compatible`
- **School policy**: sanhe-first output shaped to stay compatible with the
  `iztro` calculation contract where the Korean calendar policy permits it
- **Calendar policy**: Korean lunar calendar via `rs-klc`, aligned with
  Dalgyeol/KST and the existing saju engine
- **Primary calculation references**: open-source or primary table fixtures
  only
- **Secondary references**: multiple public traditional tables for
  cross-checking before promotion to primary calculation data
- **Interpretation policy**: Dalgyeol Korean service copy is applied only after
  calculation; prose sources must not redefine core placement tables
- **Unsupported policy**: any table without authoritative fixture coverage must
  be emitted with an explicit pending/heuristic source policy

## MVP Scope

The first engine contract is calculation-first and intentionally narrow:

- normalize solar/lunar birth date with `rs-klc`
- derive the two-hour earthly branch from birth time
- calculate life palace and body palace from lunar month and birth hour
- calculate palace stems with the five-tiger rule
- calculate the five-element bureau from the life palace stem/branch Na Yin
- place Ziwei, Tianfu, and the 14 major stars
- return both a typed `ZiweiChart` and a compatibility JSON payload

No interpretation prose, rendering, database access, network calls, or LLM calls
belong in this crate.

## Adopted Calculation Policies

Zi Wei Dou Shu has school-specific edge cases. This crate must make those
choices explicit so callers can treat output as a stable product contract.

- **Calendar system**: Dalgyeol uses the Korean lunar calendar conversion from
  `rs-klc` so Ziwei dates stay consistent with the existing saju engine and KST
  product policy. Chinese-calendar reference engines such as `iztro` can differ
  on rare leap-month years. For example, 2012 is a Korean leap-3rd-month year
  but a Chinese leap-4th-month year.
- **Leap lunar month**: MVP uses the same numeric lunar month for palace
  placement. It preserves `is_lunar_leap_month` in output, but does not shift
  leap-month births to the previous or next month.
- **Late Zi hour (23:00-23:59)**: MVP keeps the provided civil birth date and
  maps 23:00 to Zi hour. It does not shift the lunar day to the next day.
- **Invalid input**: public calculation fails instead of silently coercing
  invalid hours, minutes, or lunar days.

## JSON Contract

The typed chart uses Rust enums for SDK consumers. The compatibility JSON
expands branches, stems, palace names, and stars into objects with stable
`code`, Korean label, and Hanja fields. Backend and web callers should use the
JSON payload as the public API contract.

## Verification Standard

Smoke tests only protect response shape. Calculation changes should add fixture
tests that pin at least:

- life/body palace
- five-element bureau
- Ziwei and Tianfu branches
- all 14 major-star placements
- leap-month policy behavior
- 23:00 late-Zi policy behavior
