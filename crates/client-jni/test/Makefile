java_run: lib
	javac ExogressClient.java && java -Djava.library.path=../target/debug/ ExogressClient

.PHONY: lib

javah:
	javah ExogressClient

lib:
	cd .. && cargo build
