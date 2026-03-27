{ vmLib, pkgs, capsaPackage }:
let
  assets = vmLib.mkVMAssets { name = "capsa-daemon-no-network-fast-path"; };
  capsa = "${capsaPackage}/libexec/capsa/capsa";
  libkrunLibDir =
    if pkgs.stdenv.isDarwin then
      "${pkgs.lib.getLib pkgs."libkrun-efi"}/lib"
    else
      "${pkgs.lib.getLib pkgs.libkrun}/lib";
in
pkgs.runCommand "capsa-daemon-no-network-fast-path-vm-check"
{
  nativeBuildInputs = [ pkgs.expect pkgs.coreutils ];
}
  ''
    set -euo pipefail

    netd_stub="$TMPDIR/capsa-netd-non-executable"
    printf 'not executable\n' > "$netd_stub"
    chmod 0644 "$netd_stub"
    export NETD_STUB="$netd_stub"

    cat > vm-check.expect <<'EOF'
    #!${pkgs.expect}/bin/expect
    set timeout 20

    set capsa "${capsa}"
    set kernel "${assets.kernelImage}"
    set initramfs "${assets.initramfsImage}"
    set kernel_cmdline {${assets.vmConfig.kernelCmdline}}
    set env(LIBKRUN_LIB_DIR) "${libkrunLibDir}"
    set env(CAPSA_DISABLE_SANDBOX) 1
    set env(CAPSA_NETD_PATH) $env(NETD_STUB)

    spawn $capsa \
      --kernel $kernel \
      --initramfs $initramfs \
      --kernel-cmdline $kernel_cmdline \
      --vcpus ${toString assets.vmConfig.vcpus} \
      --memory-mib ${toString assets.vmConfig.memoryMiB}

    proc fail {message} {
      puts "ERROR: $message"
      exit 1
    }

    set vm_prompt_re {~ # }

    expect {
      -re $vm_prompt_re {}
      timeout { fail "timed out waiting for VM prompt" }
      eof { fail "capsa exited before VM prompt" }
    }

    send -- "exit\r"
    expect {
      eof {}
      timeout { fail "timed out waiting for VM shutdown" }
    }

    set wait_result [wait]
    set status [lindex $wait_result 3]
    if {$status != 0} {
      fail "capsa exited with status $status"
    }
    EOF

    chmod +x vm-check.expect
    ./vm-check.expect

    touch $out
  ''
