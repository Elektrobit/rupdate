properties([gitLabConnection('GitLab')])

pipeline {
    agent {
        label 'agent'
    }
    environment {
        UID = sh(script: 'id -u', returnStdout: true).trim()
        GID = sh(script: 'id -g', returnStdout: true).trim()
        TMPDIR = '/tmp'
    }
    options {
        gitlabBuilds(builds: [
            "Build (amd64)",
            "Build (arm64v8)",
            "Analyse: Lint (amd64)",
            "Analyse: Lint (arm64v8)",
            "Test: utests (amd64)",
            "Test: utests (arm64v8)",
        ])
        buildDiscarder(logRotator(numToKeepStr: '4'))
        disableConcurrentBuilds()
    }
    stages {
        stage ('Setup') {
            steps {
                sh '''#!/bin/bash -xe
                git clean -xdff
                '''
            }
        }
        stage ('Build') {
            matrix {
                axes {
                    axis {
                        name 'ARCH'
                        values 'amd64', 'arm64v8'
                    }
                }
                agent {
                    dockerfile {
                        dir 'ci'
                        reuseNode true
                        additionalBuildArgs "--build-arg REPO=${ARCH} --build-arg USER=jenkins \
                            --build-arg UID=${UID} --build-arg GID=${GID}"
                        args "--privileged \
                            -v /home/jenkins/.ssh:/home/jenkins/.ssh \
                            -e HOME=/home/jenkins"
                    }
                }
                stages {
                    stage('Build') {
                        steps {
                            gitlabCommitStatus("${STAGE_NAME} (${ARCH})") {
                                sh '''#!/bin/bash -xe
                                    ci/build.sh
                                    ci/build.sh debug
                                '''
                            }
                        }
                    }
                    stage ('Analyse: Lint') {
                        steps {
                            gitlabCommitStatus("${STAGE_NAME} (${ARCH})") {
                                sh '''#!/bin/bash -xe
                                    ci/run-lint.sh
                                '''
                            }
                        }
                    }
                    stage ('Test: utests') {
                        steps {
                            gitlabCommitStatus("${STAGE_NAME} (${ARCH})") {
                                sh '''#!/bin/bash -xe
                                    ci/run-utests.sh
                                '''
                            }
                        }
                    }
                }
            }
        }
    }
    post {
        always {
            archiveArtifacts 'result/**'
        }
    }
}
