FROM debian:12
RUN apt-get update && apt-get install curl
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-no-recommends
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-pin-versions
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-yes
RUN apt-get install -y curl=7.88 wget=1.21
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-no-recommends
RUN apt-get install -y --no-install-recommends curl
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-pin-versions
RUN apt-get install -qq -y -t stable curl=7.88
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-no-recommends
RUN apt-get -y install foo/stable
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-no-recommends
RUN apt-get install -yq ./a.deb
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ apt-get-no-recommends
RUN apt-get install --yes \
^ apt-get-no-recommends
^ apt-get-pin-versions
    curl \
    git
RUN apt-get update
