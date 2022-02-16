FROM openjdk:19-bullseye
RUN apt update && apt install -y python3-websockets python3-msgpack
RUN git clone https://github.com/nus-sys/cs5223-labs /usr/src/myapp
COPY ./worker.py /usr/src/myapp/worker.py
WORKDIR /usr/src/myapp
CMD python3 worker.py
