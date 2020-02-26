import numpy as np
import socket
import sys
import tensorflow as tf
import threading

from tensorflow.keras.preprocessing import image
from PIL import ImageFile
ImageFile.LOAD_TRUNCATED_IMAGES = True

IMG_HEIGHT = 350
IMG_WIDTH = 350

def handle_request(conn):
    img_path = conn.recv(26)
    if not img_path:
        return
    img = image.load_img("/home/ubuntu/task/face.jpg", target_size=(IMG_WIDTH, IMG_HEIGHT))
    img = image.img_to_array(img)
    img = np.expand_dims(img, axis=0)
    conn.sendall("{}".format(int(model.predict(img)[0][0])).encode('utf-8'))   
    conn.close()

def start_server(model, port):
    """ Start a server on port listening for inference requests. """

    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.bind(('127.0.0.1', port))
    s.listen(5)

    while True:
        conn, addr = s.accept()
        print("NEW CONNECTION");
        thread = threading.Thread(target=handle_request, args=(conn,))
        thread.start()

if __name__ == "__main__":
    if len(sys.argv) != 3:
        print("usage: python serve_model.py <saved-model-path> <port>")

    model = tf.keras.models.load_model(sys.argv[1])
    start_server(model, int(sys.argv[2]))
