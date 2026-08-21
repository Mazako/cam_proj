import cv2


BACKGROUND_LEARNING_FRAMES = 90
MIN_MOTION_AREA = 1_000


def main() -> None:
    camera = cv2.VideoCapture(0)

    if not camera.isOpened():
        raise RuntimeError("Could not open the default camera (index 0).")

    window_name = "Motion detection — Q/Esc: close, R: reset background"
    background_subtractor = cv2.createBackgroundSubtractorMOG2(
        history=500,
        varThreshold=32,
        detectShadows=True,
    )
    learning_frames_remaining = BACKGROUND_LEARNING_FRAMES

    try:
        while True:
            success, frame = camera.read()
            if not success:
                raise RuntimeError("Could not read a frame from the camera.")

            foreground_mask = background_subtractor.apply(frame)

            if learning_frames_remaining > 0:
                learning_frames_remaining -= 1
                cv2.putText(
                    frame,
                    "Learning background...",
                    (20, 40),
                    cv2.FONT_HERSHEY_SIMPLEX,
                    0.8,
                    (0, 255, 255),
                    2,
                )
            else:
                _, foreground_mask = cv2.threshold(
                    foreground_mask, 200, 255, cv2.THRESH_BINARY
                )
                kernel = cv2.getStructuringElement(cv2.MORPH_ELLIPSE, (5, 5))
                foreground_mask = cv2.morphologyEx(
                    foreground_mask, cv2.MORPH_OPEN, kernel
                )
                foreground_mask = cv2.dilate(foreground_mask, None, iterations=2)

                contours, _ = cv2.findContours(
                    foreground_mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE
                )
                motion_detected = False
                for contour in contours:
                    if cv2.contourArea(contour) < MIN_MOTION_AREA:
                        continue

                    motion_detected = True
                    x, y, width, height = cv2.boundingRect(contour)
                    cv2.rectangle(
                        frame,
                        (x, y),
                        (x + width, y + height),
                        (0, 255, 0),
                        2,
                    )

                if motion_detected:
                    cv2.putText(
                        frame,
                        "Motion detected",
                        (20, 40),
                        cv2.FONT_HERSHEY_SIMPLEX,
                        0.8,
                        (0, 0, 255),
                        2,
                    )

            mask_preview = cv2.cvtColor(foreground_mask, cv2.COLOR_GRAY2BGR)
            cv2.putText(
                frame,
                "Camera",
                (20, frame.shape[0] - 20),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.7,
                (255, 255, 255),
                2,
            )
            cv2.putText(
                mask_preview,
                "MOG2 mask",
                (20, mask_preview.shape[0] - 20),
                cv2.FONT_HERSHEY_SIMPLEX,
                0.7,
                (255, 255, 255),
                2,
            )
            combined_frame = cv2.hconcat([frame, mask_preview])
            cv2.imshow(window_name, combined_frame)
            key = cv2.waitKey(1) & 0xFF
            if key in (ord("q"), 27):
                break
            if key == ord("r"):
                background_subtractor = cv2.createBackgroundSubtractorMOG2(
                    history=500,
                    varThreshold=32,
                    detectShadows=True,
                )
                learning_frames_remaining = BACKGROUND_LEARNING_FRAMES
    finally:
        camera.release()
        cv2.destroyAllWindows()


if __name__ == "__main__":
    main()
