FROM node:20
RUN pip install flask
^^^^^^^^^^^^^^^^^^^^^ pip-pin-versions
RUN pip install flask==2.0 "requests>=2"
RUN pip3 install -r requirements.txt
RUN pip install --no-cache-dir -i https://x/simple flask==1
RUN pip install .
RUN pip install git+https://github.com/a/b
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ pip-pin-versions
RUN pip install git+https://github.com/a/b@v1
RUN npm install
RUN npm install express
^^^^^^^^^^^^^^^^^^^^^^^ npm-pin-versions
RUN npm install express@4 @scope/pkg@1
RUN npm install --loglevel verbose express@4
RUN npm install ./local
RUN gem install rails
^^^^^^^^^^^^^^^^^^^^^ gem-pin-versions
RUN gem install rails:7.0
RUN gem install rails -v 7.0
RUN go install example.com/x
^^^^^^^^^^^^^^^^^^^^^^^^^^^^ go-pin-versions
RUN go install example.com/x@v1
RUN go install example.com/x@latest
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ go-pin-versions
