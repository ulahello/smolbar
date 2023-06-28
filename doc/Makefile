.PHONY: all
all: smolbar.1 smolbar.5

smolbar.1: smolbar.1.scd
	scdoc < smolbar.1.scd > smolbar.1

smolbar.5: smolbar.5.scd
	scdoc < smolbar.5.scd > smolbar.5

.PHONY: clean
clean:
	rm -vf smolbar.1 smolbar.5
