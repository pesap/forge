# Changelog

## [0.7.1](https://github.com/pesap/forge/compare/v0.7.0...v0.7.1) (2026-06-27)


### Bug Fixes

* simplify generated commitizen-branch hook ([#131](https://github.com/pesap/forge/issues/131)) ([af0d126](https://github.com/pesap/forge/commit/af0d12605353d00ac0354a938f1e10ea422889b4))
* **sync:** show managed diffs before interactive apply ([#128](https://github.com/pesap/forge/issues/128)) ([e19a9b4](https://github.com/pesap/forge/commit/e19a9b48c5970ec2a7a921e455287f201bdd5027))

## [0.7.0](https://github.com/pesap/forge/compare/v0.6.1...v0.7.0) (2026-06-25)


### Features

* **python-library:** generate safe commitizen hooks ([#121](https://github.com/pesap/forge/issues/121)) ([cf5faa8](https://github.com/pesap/forge/commit/cf5faa8989dac2f78ae820e1d728e0070818a093))
* **sync:** preserve external platform infrastructure ([#125](https://github.com/pesap/forge/issues/125)) ([918fa3e](https://github.com/pesap/forge/commit/918fa3ec4aafa9fbc1b93c939ab02cb3bc31aa57))


### Bug Fixes

* **templates:** preserve CI branch and option choices ([#126](https://github.com/pesap/forge/issues/126)) ([069bc56](https://github.com/pesap/forge/commit/069bc56666e9884e5a86ed5f680bd834bf0f7a04))


### Refactors

* **tests:** simplify commitizen hook setup ([#123](https://github.com/pesap/forge/issues/123)) ([aa6cfcf](https://github.com/pesap/forge/commit/aa6cfcfb8d9488cba8b060a8ae4e289de8e20a8a))


### Chores

* **deps:** bump http from 1.4.1 to 1.4.2 ([#120](https://github.com/pesap/forge/issues/120)) ([d5656bd](https://github.com/pesap/forge/commit/d5656bd0d724f5c1e8d87ee4a1c820772d4f5242))
* **deps:** bump minijinja from 2.20.0 to 2.21.0 ([#124](https://github.com/pesap/forge/issues/124)) ([2f74d14](https://github.com/pesap/forge/commit/2f74d14b66b5f7cad1107a8fb036fba85d5dde1b))

## [0.6.1](https://github.com/pesap/forge/compare/v0.6.0...v0.6.1) (2026-06-12)


### Bug Fixes

* **init:** preserve existing docs and workflow infra ([#116](https://github.com/pesap/forge/issues/116)) ([adce8b5](https://github.com/pesap/forge/commit/adce8b5208365de59dece777d0fdcbe9858d5a16))

## [0.6.0](https://github.com/pesap/forge/compare/v0.5.1...v0.6.0) (2026-06-11)


### Features

* **init:** make existing-project adoption safe ([#113](https://github.com/pesap/forge/issues/113)) ([ecd8330](https://github.com/pesap/forge/commit/ecd8330ff2393514c798a22a4bd18a1614cae575))

## [0.5.1](https://github.com/pesap/forge/compare/v0.5.0...v0.5.1) (2026-06-09)


### Bug Fixes

* stop installing forge in generated Python CI ([#102](https://github.com/pesap/forge/issues/102)) ([79a05b8](https://github.com/pesap/forge/commit/79a05b8d2b68ff37b9e02157ed50fe641d689a91))

## [0.5.0](https://github.com/pesap/forge/compare/v0.4.1...v0.5.0) (2026-06-08)


### Features

* cross-platform generated repo defaults ([#98](https://github.com/pesap/forge/issues/98)) ([0e50ae9](https://github.com/pesap/forge/commit/0e50ae947c01db5c5b6f8f2a2f652c0dd4fc931d))
* improve cross-platform generated defaults ([#100](https://github.com/pesap/forge/issues/100)) ([d76eab1](https://github.com/pesap/forge/commit/d76eab15fdbcb597d2c2eec491886a0409dbf567))


### Bug Fixes

* align line-ending hook with CRLF scripts ([#99](https://github.com/pesap/forge/issues/99)) ([c7fc916](https://github.com/pesap/forge/commit/c7fc9165f9375e2cf27434182f6932b3b3999965))
* avoid Windows drift for managed Claude link ([#96](https://github.com/pesap/forge/issues/96)) ([4596abe](https://github.com/pesap/forge/commit/4596abe1c68153f849927d775335ba9bc5edfb87))

## [0.4.1](https://github.com/pesap/forge/compare/v0.4.0...v0.4.1) (2026-06-05)


### Bug Fixes

* **python:** avoid literal XDG_CACHE_HOME pytest cache folders ([#84](https://github.com/pesap/forge/issues/84)) ([380c658](https://github.com/pesap/forge/commit/380c6588effd058c31782163e6fca3a9e62e8d46))
* **python:** generated docs fail in Starlight head merge ([#80](https://github.com/pesap/forge/issues/80)) ([0dd690a](https://github.com/pesap/forge/commit/0dd690a54eb7afb876c4d04fb6f80cd6dd82cc1e))
* **python:** verify pretty-format-json prek builtin hook ([#87](https://github.com/pesap/forge/issues/87)) ([0baa713](https://github.com/pesap/forge/commit/0baa713d625d9b8fe1767fbe21209abd4deb9de2))


### Tests

* **prettier:** assert managed prettierignore generation ([#73](https://github.com/pesap/forge/issues/73)) ([#86](https://github.com/pesap/forge/issues/86)) ([2219c76](https://github.com/pesap/forge/commit/2219c767b1d5e20dc03aa86cb2375c1d3971d3cd))


### Chores

* add managed gitattributes line-ending policy ([#74](https://github.com/pesap/forge/issues/74)) ([#85](https://github.com/pesap/forge/issues/85)) ([9a97382](https://github.com/pesap/forge/commit/9a97382b8a41632dadaf3e3518de7db4775e7da2))
* **python:** generate .typos.toml config ([#78](https://github.com/pesap/forge/issues/78)) ([#81](https://github.com/pesap/forge/issues/81)) ([f4a2a07](https://github.com/pesap/forge/commit/f4a2a0734cc3d72b05d66420747fe7fed208d957))
* **release:** move release-please config under .github ([#76](https://github.com/pesap/forge/issues/76)) ([#83](https://github.com/pesap/forge/issues/83)) ([2ce4738](https://github.com/pesap/forge/commit/2ce47383132e814eb11d8f731c6436a71e9050ac))
* remove forge sync from generated verify recipes ([#77](https://github.com/pesap/forge/issues/77)) ([#82](https://github.com/pesap/forge/issues/82)) ([ded2fe1](https://github.com/pesap/forge/commit/ded2fe1f3e2b7509ef09ee3f3c3a65f23c6dad93))

## [0.4.0](https://github.com/pesap/forge/compare/v0.3.2...v0.4.0) (2026-06-05)


### Features

* **license:** expand library license selection ([#68](https://github.com/pesap/forge/issues/68)) ([5876597](https://github.com/pesap/forge/commit/587659767503001d48edb4f6a9ca7b52b5c4d3a6))


### Bug Fixes

* **python:** sync generated project tooling ([#71](https://github.com/pesap/forge/issues/71)) ([85bb70e](https://github.com/pesap/forge/commit/85bb70eadc511086ee75414b5cee0d8c4bebec78))


### Documentation

* **agents:** refine generated guidance ([#70](https://github.com/pesap/forge/issues/70)) ([cf55b23](https://github.com/pesap/forge/commit/cf55b23d634b28214693c5b520181b36400717a8))

## [0.3.2](https://github.com/pesap/forge/compare/v0.3.1...v0.3.2) (2026-06-04)


### Bug Fixes

* **init:** preserve existing pyproject adoption ([#66](https://github.com/pesap/forge/issues/66)) ([395f362](https://github.com/pesap/forge/commit/395f362ea7cdb3f39bf9c8df5935078ce9605e72))

## [0.3.1](https://github.com/pesap/forge/compare/v0.3.0...v0.3.1) (2026-06-04)


### Bug Fixes

* **init:** minimize adopted python metadata ([#64](https://github.com/pesap/forge/issues/64)) ([31b550e](https://github.com/pesap/forge/commit/31b550e4b335a495a1830f6deb8f9420c54ddb30))

## [0.3.0](https://github.com/pesap/forge/compare/v0.2.0...v0.3.0) (2026-06-04)


### Features

* **init:** adopt existing project metadata ([#62](https://github.com/pesap/forge/issues/62)) ([399a7f7](https://github.com/pesap/forge/commit/399a7f78ae2dc3cb685c2ef8aaac8edb00f71e43))

## [0.2.0](https://github.com/pesap/forge/compare/v0.1.1...v0.2.0) (2026-06-02)


### Features

* adding `forge sync` command ([#58](https://github.com/pesap/forge/issues/58)) ([de07bb8](https://github.com/pesap/forge/commit/de07bb8ad4ff8efcb6d53773601960e748948fe1))


### Bug Fixes

* address sync follow-up naming ([#60](https://github.com/pesap/forge/issues/60)) ([e93dcaa](https://github.com/pesap/forge/commit/e93dcaae11d5b58bd609e5ca580b7d6b81901c4b))

## [0.1.1](https://github.com/pesap/forge/compare/v0.1.0...v0.1.1) (2026-06-02)


### Bug Fixes

* fixg xdg_path and path of binaries ([#56](https://github.com/pesap/forge/issues/56)) ([79612f3](https://github.com/pesap/forge/commit/79612f3ed725d6ad7650f82b05cbed259dd68cc3))

## 0.1.0 (2026-06-02)


### Chores

* initial repository state. ([1d9dbd5](https://github.com/pesap/forge/commit/1d9dbd5fc028b4ee9a96526e8c2b08960e39a858))
