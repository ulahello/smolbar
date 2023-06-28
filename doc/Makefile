PREFIX?=/usr/local
MANDIR?=$(PREFIX)/share/man

all: smolbar.1 smolbar.5

smolbar.1: smolbar.1.scd
	scdoc < $< > $@

smolbar.5: smolbar.5.scd
	scdoc < $< > $@

install: all
	mkdir -p $(DESTDIR)/$(MANDIR)/man1 $(DESTDIR)/$(MANDIR)/man5
	install -m644 smolbar.1 $(DESTDIR)/$(MANDIR)/man1/smolbar.1
	install -m644 smolbar.5 $(DESTDIR)/$(MANDIR)/man5/smolbar.5

uninstall:
	rm -f $(DESTDIR)/$(MANDIR)/man1/smolbar.1
	rm -f $(DESTDIR)/$(MANDIR)/man5/smolbar.5

clean:
	rm -vf smolbar.1 smolbar.5

.PHONY: all clean install
