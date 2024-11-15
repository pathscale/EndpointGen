# Example project

## To run

- `cargo install --git https://github.com/pathscale/EndpointGen.git --tag v0.4.4`, ideally latest tag

- `endpoint-gen <optional>--config-dir CONFIG_DIR <optional>--output-dir OUTPUT_DIR`

EndpointGen will create directories and files in the given output dir, or the current dir if none is supplied.

Please ensure that a version.toml is placed in the config dir to specify the version requirements of the binary.
The version adheres to the standard semver format