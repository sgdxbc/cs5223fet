FROM openjdk:19-bullseye
RUN apt-get update && apt-get install -y make python3-websockets python3-msgpack
RUN git clone https://github.com/nus-sys/cs5223-labs /usr/src/myapp
COPY ./worker.py /usr/src/myapp/worker.py
WORKDIR /usr/src/myapp
CMD python3 worker.py
