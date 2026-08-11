<div class="stage">
	<span class="dot" aria-hidden="true"></span>
</div>

<style>
	.stage {
		display: flex;
		align-items: flex-end;
		justify-content: flex-end;
		width: 100vw;
		height: 100vh;
		padding: 0 6px 6px 0;
		box-sizing: border-box;
	}

	.dot {
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: oklch(0.511 0.096 186.391);
		box-shadow:
			0 0 0 1px rgba(255, 255, 255, 0.85),
			0 0 0 2px rgba(0, 0, 0, 0.38),
			0 1px 3px rgba(0, 0, 0, 0.28);
		animation: pulse 1.8s ease-in-out infinite;
	}

	@keyframes pulse {
		0%,
		100% {
			opacity: 1;
		}
		50% {
			opacity: 0.42;
		}
	}

	@media (prefers-reduced-motion: reduce) {
		.dot {
			animation: none !important;
		}
	}
</style>
