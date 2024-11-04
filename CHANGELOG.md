# Changelog

## Sliding Tree 0.2.0 (2024-11-04)

### Added
- Added `HasChildren` and `HasChildrenMut` traits.
- Added `NodeChildrenMut` type.

### Changed
- Changed pending roots to be handled automatically.
- Changed ordering of `set_children_subtree`'s builder parameters.
- Improved buffer occupancy.

### Fixed
- Fixed soundness issue with recursive allocation.

## Sliding Tree 0.1.0 (2024-10-25)
- Initial release
