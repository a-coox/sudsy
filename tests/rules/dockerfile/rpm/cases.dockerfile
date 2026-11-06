FROM fedora:40
RUN yum install curl
^^^^^^^^^^^^^^^^^^^^ rpm-pin-versions
^^^^^^^^^^^^^^^^^^^^ yum-clean
^^^^^^^^^^^^^^^^^^^^ yum-yes
RUN yum install -y curl-7.0 && yum clean all
RUN yum install -y curl-7.0 && rm -rf /var/cache/yum/*
RUN yum install -y nginx-1.2 && yum clean all
RUN dnf install curl
^^^^^^^^^^^^^^^^^^^^ dnf-clean
^^^^^^^^^^^^^^^^^^^^ dnf-yes
^^^^^^^^^^^^^^^^^^^^ rpm-pin-versions
RUN dnf install -y curl-7.0
^^^^^^^^^^^^^^^^^^^^^^^^^^^ dnf-clean
RUN dnf install -y curl-7.0 && dnf clean all
RUN microdnf install -y curl-7.0
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ dnf-clean
RUN dnf install -y ./a.rpm && dnf clean all
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ rpm-pin-versions
RUN zypper install curl
^^^^^^^^^^^^^^^^^^^^^^^ zypper-clean
^^^^^^^^^^^^^^^^^^^^^^^ zypper-pin-versions
^^^^^^^^^^^^^^^^^^^^^^^ zypper-yes
RUN zypper install -y curl=7.0 && zypper clean
RUN zypper install -y curl
^^^^^^^^^^^^^^^^^^^^^^^^^^ zypper-clean
^^^^^^^^^^^^^^^^^^^^^^^^^^ zypper-pin-versions
RUN zypper dist-upgrade
^^^^^^^^^^^^^^^^^^^^^^^ zypper-no-dist-upgrade
RUN zypper -n in curl=1 && zypper cc
