FROM alpine:3
RUN sudo make install
^^^^^^^^^^^^^^^^^^^^^ no-sudo
RUN cd /app && make
^^^^^^^^^^^^^^^^^^^ use-workdir-not-cd
RUN service nginx start
^^^^^^^^^^^^^^^^^^^^^^^ no-interactive-commands
RUN apt update
^^^^^^^^^^^^^^ no-apt
RUN wget https://example.com/f
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ wget-progress
RUN wget -q https://example.com/f
RUN ln -sf /bin/bash /bin/sh
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-shell-symlink
RUN echo sudo cd ps
RUN make && \
^ no-sudo
    sudo make install
RUN v=$(sudo make) && echo "$(wget https://example.com/f)"
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ no-sudo
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ wget-progress
ONBUILD RUN sudo make
#       ^^^^^^^^^^^^^ no-sudo
RUN ["sudo", "make"]
^^^^^^^^^^^^^^^^^^^^ no-sudo
