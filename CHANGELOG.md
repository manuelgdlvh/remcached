# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## 0.2.0 - 2024-12-24

### Added

- Add a merge algorithm to related cache tasks. For example, if two invalidations or expirations are created for the
  same cache entity, the Cache Manager will discard the oldest one.
  The task Key must implement the ToString trait to enable the merge. The merge will be based on the cache identifier,
  Operation type, and the input identifier.
- Add new statistics for Cache Manager:
    * Number of tasks merged
- Add input validators for manual operations in the Cache Manager.
  Since manual operations are performed from a centralized point for all caches, the inputs for these operations are
  generic. Input validation has been added, taking into account the cache and the type of operation.
  The error types for manual operations are included in the ManualOperationError enum.
- Add integration tests for the new use cases

### Fixed

- Handling of types in manual operations in the Cache Manager to prevent errors in the core processing and return them
  earlier.

## 0.1.0 - 2024-12-22

### Added

- Initial version

### Fixed

[@manuelgdlvh]: https://github.com/manuelgdlvh
