#!/usr/bin/env python3
"""Build a Switch ELF and convert it to NRO; keep reviewed Releases unchanged."""
from pathlib import Path
import argparse, json, os, subprocess

def main():
    p=argparse.ArgumentParser()
    p.add_argument('--toolchain', default='cobalt-zip-audio', help='Configured Switch Rust toolchain alias')
    p.add_argument('--host-toolchain', default='nightly-2026-02-14', help='Host Rust compiler for wrapper and ELF-to-NRO converter')
    p.add_argument('--offline', action='store_true')
    args=p.parse_args()
    root=Path(__file__).resolve().parents[1]
    tools=root/'tools'; out=root/'dist'; out.mkdir(exist_ok=True)
    environment=os.environ.copy()
    environment['CARGO_TARGET_DIR']=str(root/'target')
    wrapper=out/('rustc-wrapper.exe' if os.name=='nt' else 'rustc-wrapper')
    subprocess.run(['rustc', '+'+args.host_toolchain, str(tools/'rustc-wrapper.rs'), '-o', str(wrapper)], check=True)
    environment['RUSTC_WRAPPER']=str(wrapper)
    template=json.loads((tools/'aarch64-skyline-switch.json').read_text())
    template['pre-link-args']['ld.lld'][0]='-T'+(tools/'link.T').as_posix()
    target=out/'aarch64-skyline-switch.json'
    target.write_text(json.dumps(template, indent=2), encoding='utf-8')
    command=['cargo', '+'+args.toolchain, 'build', '--manifest-path', str(root/'Cargo.toml'), '--release', '--locked', '-Z', 'json-target-spec', '-Z', 'build-std=std,panic_abort', '--target', str(target)]
    if args.offline: command.append('--offline')
    subprocess.run(command, cwd=root, env=environment, check=True)
    env_host=os.environ.copy(); env_host.pop('RUSTC_WRAPPER', None); env_host['CARGO_TARGET_DIR']=str(root/'target/host-tools')
    host=['cargo', '+'+args.host_toolchain, 'build', '--manifest-path', str(tools/'nro-converter/Cargo.toml'), '--release', '--locked']
    if args.offline: host.append('--offline')
    subprocess.run(host, cwd=root, env=env_host, check=True)
    converter=root/'target/host-tools/release'/('cobalt-nro-converter.exe' if os.name=='nt' else 'cobalt-nro-converter')
    name=json.loads((root/'project.json').read_text())['crate_name']
    elf=root/'target/aarch64-skyline-switch/release'/('lib'+name+'.so')
    nro=out/(name+'.nro')
    subprocess.run([str(converter), str(elf), str(nro)], check=True)
    data=nro.read_bytes()
    if data[16:20]!=b'NRO0': raise RuntimeError('Invalid NRO header')
    print(nro)

if __name__=='__main__': main()
