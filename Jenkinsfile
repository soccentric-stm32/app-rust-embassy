// Jenkinsfile - multi-stage CI for the STM32 Rust (Embassy) application.
//
//   Checkout -> Toolchain -> Build -> Lint -> Format -> Image -> Publish
//
// Builds run through scripts/docker-dev.sh (RCON-001). The Rust image is
// built on top of the shared stm32-dev base, so the Toolchain stage creates
// the base first when the agent does not have it yet.

pipeline {
    agent any

    options {
        skipDefaultCheckout()
        timestamps()
        ansiColor('xterm')
        timeout(time: 45, unit: 'MINUTES')
        buildDiscarder(logRotator(numToKeepStr: '30', artifactNumToKeepStr: '10'))
        disableConcurrentBuilds()
    }

    parameters {
        booleanParam(name: 'RUN_LINT', defaultValue: true, description: 'Run cargo clippy -D warnings')
        booleanParam(name: 'CHECK_FORMAT', defaultValue: true, description: 'Check cargo fmt (marks the build unstable on drift)')
    }

    environment {
        BASE_IMAGE  = 'stm32-dev:latest'
        STM32_IMAGE = 'stm32-dev-rust:latest'
    }

    stages {
        stage('Checkout') {
            steps {
                checkout scm
                sh '''
                    git log -1 --pretty=fuller
                    echo "commit: $(git rev-parse HEAD)"
                    echo "branch: $(git rev-parse --abbrev-ref HEAD)"
                '''
            }
        }

        stage('Toolchain') {
            steps {
                sh '''
                    docker version --format 'docker server: {{.Server.Version}}'
                    if ! docker image inspect "${BASE_IMAGE}" >/dev/null 2>&1; then
                        echo "Building ${BASE_IMAGE} from docker/Dockerfile.base"
                        docker build -t "${BASE_IMAGE}" -f docker/Dockerfile.base docker
                    fi
                    if ! docker image inspect "${STM32_IMAGE}" >/dev/null 2>&1; then
                        echo "Building ${STM32_IMAGE} from docker/Dockerfile"
                        make image
                    fi
                    docker image inspect --format 'base:  {{.Id}}' "${BASE_IMAGE}"
                    docker image inspect --format 'image: {{.Id}}' "${STM32_IMAGE}"
                    scripts/docker-dev.sh run rustc --version
                    scripts/docker-dev.sh run cargo --version
                '''
            }
        }

        stage('Build') {
            steps {
                sh 'make build'
            }
        }

        stage('Lint') {
            when { expression { return params.RUN_LINT } }
            steps {
                sh 'make lint'
            }
        }

        stage('Format') {
            when { expression { return params.CHECK_FORMAT } }
            steps {
                // rustfmt drift is reported, but does not block a firmware build.
                catchError(buildResult: 'UNSTABLE', stageResult: 'FAILURE') {
                    sh 'scripts/docker-dev.sh run env CARGO_HOME=/work/.cargo-home cargo fmt --all -- --check'
                }
            }
        }

        stage('Image') {
            steps {
                sh '''
                    make size
                    python3 - build/rust-app.img <<'PY'
import struct
import sys
import zlib
from pathlib import Path

path = Path(sys.argv[1])
data = path.read_bytes()
header = struct.unpack_from("<IHHIIIIIII", data, 0)
magic, hver, hsize, itype, ver, isize, icrc, load, entry, _ts = header
assert magic == 0x48574249, "bad image magic"
assert hver == 1, f"unexpected header version {hver}"
assert hsize == 256, f"unexpected header size {hsize}"
assert itype == 1, f"unexpected image type {itype}"
assert load == 0x0800C000, f"image loads at 0x{load:08X}, expected 0x0800C000"
assert len(data) == hsize + isize, "image size does not match the header"
assert zlib.crc32(data[hsize:]) & 0xFFFFFFFF == icrc, "image CRC32 mismatch"
sp = struct.unpack_from("<I", data, hsize)[0]
assert 0x20000000 < sp <= 0x20080000, "initial stack pointer is outside internal RAM"
entry_ok = entry & 1 and 0x0800C000 <= (entry & ~1) < 0x0800C000 + 0x000F4000
assert entry_ok, "reset vector is outside the slot"
print(f"image: {path} v{(ver >> 24) & 0xFF}.{(ver >> 16) & 0xFF}.{ver & 0xFF} "
      f"({isize} bytes payload, entry 0x{entry:08X})")
PY
                '''
            }
        }

        stage('Publish') {
            steps {
                sh '''
                    sha256sum build/rust-app.elf build/rust-app.bin build/rust-app.hex > build/SHA256SUMS
                    sha256sum build/rust-app.img >> build/SHA256SUMS
                    cat build/SHA256SUMS
                '''
                archiveArtifacts artifacts: 'build/rust-app.elf,build/rust-app.bin,build/rust-app.hex,build/rust-app.img,build/SHA256SUMS',
                                 fingerprint: true
            }
        }
    }

    post {
        always {
            deleteDir()
        }
        success {
            echo 'STM32 Rust application: build, clippy and image checks passed.'
        }
        failure {
            echo 'STM32 Rust application: pipeline failed - see the stage logs above.'
        }
    }
}
