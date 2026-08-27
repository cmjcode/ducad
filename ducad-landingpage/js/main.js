(() => {
  "use strict";

  const prefersReducedMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  /* ---------- Sticky navbar shadow on scroll ---------- */
  const navbar = document.getElementById("navbar");
  const onScroll = () => {
    if (!navbar) return;
    navbar.classList.toggle("is-scrolled", window.scrollY > 8);

    const toTop = document.getElementById("toTop");
    if (toTop) toTop.classList.toggle("is-visible", window.scrollY > 480);
  };
  document.addEventListener("scroll", onScroll, { passive: true });
  onScroll();

  /* ---------- Mobile nav toggle ---------- */
  const navToggle = document.getElementById("navToggle");
  const primaryNav = document.getElementById("primary-nav");
  if (navToggle && primaryNav) {
    navToggle.addEventListener("click", () => {
      const isOpen = document.body.classList.toggle("nav-open");
      navToggle.setAttribute("aria-expanded", String(isOpen));
    });

    primaryNav.querySelectorAll("a").forEach((link) => {
      link.addEventListener("click", () => {
        document.body.classList.remove("nav-open");
        navToggle.setAttribute("aria-expanded", "false");
      });
    });
  }

  /* ---------- Back to top ---------- */
  const toTopBtn = document.getElementById("toTop");
  if (toTopBtn) {
    toTopBtn.addEventListener("click", () => {
      window.scrollTo({ top: 0, behavior: prefersReducedMotion ? "auto" : "smooth" });
    });
  }

  /* ---------- Reveal on scroll ---------- */
  const revealEls = document.querySelectorAll(".reveal");
  if (revealEls.length) {
    if (prefersReducedMotion || !("IntersectionObserver" in window)) {
      revealEls.forEach((el) => el.classList.add("is-visible"));
    } else {
      const revealNow = (el) => el.classList.add("is-visible");

      const observer = new IntersectionObserver(
        (entries) => {
          entries.forEach((entry) => {
            if (entry.isIntersecting) {
              revealNow(entry.target);
              observer.unobserve(entry.target);
            }
          });
        },
        { threshold: 0.05, rootMargin: "0px 0px -10% 0px" }
      );
      revealEls.forEach((el) => observer.observe(el));

      // Safety net: a large single-frame scroll jump (scrollbar drag, instant
      // anchor navigation, programmatic scrollTo) can move an element through
      // the viewport without the observer ever reporting an intersection,
      // leaving it permanently at opacity:0. Force-reveal anything still
      // hidden a few seconds after load so content is never lost.
      window.addEventListener("load", () => {
        setTimeout(() => {
          document.querySelectorAll(".reveal:not(.is-visible)").forEach(revealNow);
        }, 2500);
      });
    }
  }

  /* ---------- Copy-to-clipboard for code block ---------- */
  document.querySelectorAll(".copy-btn").forEach((btn) => {
    btn.addEventListener("click", async () => {
      const targetId = btn.getAttribute("data-copy-target");
      const target = targetId ? document.getElementById(targetId) : null;
      if (!target) return;

      const text = target.innerText;
      const originalLabel = btn.textContent;
      const i18n = window.ducadI18n;
      const copiedKey = btn.getAttribute("data-i18n-copied");
      const restoreKey = btn.getAttribute("data-i18n");

      try {
        await navigator.clipboard.writeText(text);
      } catch (err) {
        // Clipboard API unavailable (older browser / insecure context) — fall back silently.
        const textarea = document.createElement("textarea");
        textarea.value = text;
        textarea.style.position = "fixed";
        textarea.style.opacity = "0";
        document.body.appendChild(textarea);
        textarea.select();
        try {
          document.execCommand("copy");
        } catch (fallbackErr) {
          // Nothing more we can do — leave the button label unchanged.
          document.body.removeChild(textarea);
          return;
        }
        document.body.removeChild(textarea);
      }

      btn.textContent = (copiedKey && i18n && i18n.t(copiedKey)) || "Copied ✓";
      btn.classList.add("is-copied");
      setTimeout(() => {
        btn.textContent = (restoreKey && i18n && i18n.t(restoreKey)) || originalLabel;
        btn.classList.remove("is-copied");
      }, 2000);
    });
  });
})();
